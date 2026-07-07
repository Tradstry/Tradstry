// OAuth 2.0 Authorization Code + PKCE against Clerk, run entirely in Rust.
// The system browser handles the actual login (email or social); we catch the
// redirect on a fixed loopback port, exchange the code for tokens, and store
// them in the OS keychain. Tokens never touch the webview/JS.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, CsrfToken, EndpointNotSet,
    EndpointSet, PkceCodeChallenge, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};

// Client type after auth + token endpoints are set (redirect doesn't affect the
// type-state). Generics: HasAuthUrl, HasDeviceAuthUrl, HasIntrospectionUrl,
// HasRevocationUrl, HasTokenUrl.
type ConfiguredClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;
use serde::{Deserialize, Serialize};

const CLIENT_ID: &str = env!("CLERK_OAUTH_CLIENT_ID");
const PUBLISHABLE_KEY: &str = env!("VITE_CLERK_PUBLISHABLE_KEY");

// Fixed loopback port so the redirect URI registered in Clerk stays stable.
// This exact URL must be added to the Clerk OAuth application's redirect URIs.
const REDIRECT_PORT: u16 = 8788;

const KEYCHAIN_SERVICE: &str = "com.user.tradstry.auth";
const KEYCHAIN_ACCOUNT: &str = "clerk-oauth";

// Self-contained page shown in the browser after the redirect is caught.
// No external resources; theme-aware; matches the app's design language.
const SUCCESS_HTML: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Signed in · Tradstry</title>
<style>
  :root { color-scheme: light dark; }
  * { margin: 0; box-sizing: border-box; }
  body {
    min-height: 100dvh;
    display: grid;
    place-items: center;
    padding: 24px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    background: #fafafa;
    color: #18181b;
    -webkit-font-smoothing: antialiased;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 22px;
    max-width: 320px;
    animation: rise 420ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  .badge {
    width: 60px;
    height: 60px;
    border-radius: 9999px;
    display: grid;
    place-items: center;
    background: rgba(16, 185, 129, 0.12);
    animation: pop 440ms cubic-bezier(0.34, 1.56, 0.64, 1) 90ms both;
  }
  .badge svg {
    width: 30px;
    height: 30px;
    stroke: #10b981;
  }
  .badge path {
    stroke-dasharray: 30;
    stroke-dashoffset: 30;
    animation: draw 420ms ease-out 340ms forwards;
  }
  .copy { display: flex; flex-direction: column; gap: 7px; }
  h1 { font-size: 20px; font-weight: 600; letter-spacing: -0.012em; }
  p { font-size: 14px; line-height: 1.55; color: #71717a; }
  .brand {
    margin-top: 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #d4d4d8;
  }
  @media (prefers-color-scheme: dark) {
    body { background: #09090b; color: #fafafa; }
    .badge { background: rgba(52, 211, 153, 0.14); }
    .badge svg { stroke: #34d399; }
    p { color: #a1a1aa; }
    .brand { color: #3f3f46; }
  }
  @keyframes rise { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
  @keyframes pop { from { opacity: 0; transform: scale(0.82); } to { opacity: 1; transform: none; } }
  @keyframes draw { to { stroke-dashoffset: 0; } }
  @media (prefers-reduced-motion: reduce) {
    .card, .badge { animation: none; }
    .badge path { animation: none; stroke-dashoffset: 0; }
  }
</style>
</head>
<body>
  <main class="card">
    <div class="badge">
      <svg viewBox="0 0 24 24" fill="none" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M20 6 9 17l-5-5"/>
      </svg>
    </div>
    <div class="copy">
      <h1>Signed in to Tradstry</h1>
      <p>You're all set — close this tab and head back to the app.</p>
    </div>
    <div class="brand">Tradstry</div>
  </main>
</body>
</html>"##;

#[derive(Serialize, Deserialize, Clone)]
struct StoredTokens {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: u64,
    email: Option<String>,
    name: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    signed_in: bool,
    email: Option<String>,
    name: Option<String>,
}

impl AuthStatus {
    fn signed_out() -> Self {
        Self { signed_in: false, email: None, name: None }
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The Clerk instance base URL is encoded in the publishable key:
/// `pk_(test|live)_<base64("<host>$")>`.
fn clerk_base_url() -> String {
    let payload = PUBLISHABLE_KEY
        .strip_prefix("pk_test_")
        .or_else(|| PUBLISHABLE_KEY.strip_prefix("pk_live_"))
        .unwrap_or(PUBLISHABLE_KEY);
    let host = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|s| s.trim_end_matches('$').to_string())
        .unwrap_or_default();
    format!("https://{host}")
}

fn redirect_uri() -> String {
    format!("http://localhost:{REDIRECT_PORT}/callback")
}

fn oauth_client() -> Result<ConfiguredClient, String> {
    let base = clerk_base_url();
    Ok(BasicClient::new(ClientId::new(CLIENT_ID.to_string()))
        .set_auth_uri(AuthUrl::new(format!("{base}/oauth/authorize")).map_err(|e| e.to_string())?)
        .set_token_uri(TokenUrl::new(format!("{base}/oauth/token")).map_err(|e| e.to_string())?)
        .set_redirect_uri(RedirectUrl::new(redirect_uri()).map_err(|e| e.to_string())?))
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())
}

// --- Keychain -------------------------------------------------------------

fn keychain_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT).map_err(|e| e.to_string())
}

fn save_tokens(tokens: &StoredTokens) -> Result<(), String> {
    let json = serde_json::to_string(tokens).map_err(|e| e.to_string())?;
    keychain_entry()?.set_password(&json).map_err(|e| e.to_string())
}

fn load_tokens() -> Option<StoredTokens> {
    let json = keychain_entry().ok()?.get_password().ok()?;
    serde_json::from_str(&json).ok()
}

fn clear_tokens() -> Result<(), String> {
    match keychain_entry()?.delete_credential() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

// --- User info ------------------------------------------------------------

async fn fetch_userinfo(base: &str, access_token: &str) -> (Option<String>, Option<String>) {
    let Ok(client) = http_client() else {
        return (None, None);
    };
    let resp = client
        .get(format!("{base}/oauth/userinfo"))
        .bearer_auth(access_token)
        .send()
        .await;
    let Ok(v) = (match resp {
        Ok(r) => r.json::<serde_json::Value>().await,
        Err(_) => return (None, None),
    }) else {
        return (None, None);
    };
    let email = v.get("email").and_then(|x| x.as_str()).map(String::from);
    let name = v
        .get("name")
        .and_then(|x| x.as_str())
        .map(String::from)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            let given = v.get("given_name").and_then(|x| x.as_str()).unwrap_or("");
            let family = v.get("family_name").and_then(|x| x.as_str()).unwrap_or("");
            let full = format!("{given} {family}").trim().to_string();
            (!full.is_empty()).then_some(full)
        });
    (email, name)
}

// --- Commands -------------------------------------------------------------

#[tauri::command]
pub async fn sign_in() -> Result<AuthStatus, String> {
    let base = clerk_base_url();
    let client = oauth_client()?;

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // Loopback server catches the redirect; hand the URL back over a channel.
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let config = tauri_plugin_oauth::OauthConfig {
        ports: Some(vec![REDIRECT_PORT]),
        response: Some(SUCCESS_HTML.into()),
    };
    let tx_cb = tx.clone();
    let port = tauri_plugin_oauth::start_with_config(config, move |url| {
        if let Ok(mut guard) = tx_cb.lock() {
            if let Some(sender) = guard.take() {
                let _ = sender.send(url);
            }
        }
    })
    .map_err(|e| e.to_string())?;

    open::that(authorize_url.as_str()).map_err(|e| e.to_string())?;

    let redirect = tokio::time::timeout(Duration::from_secs(300), rx)
        .await
        .map_err(|_| "Sign-in timed out".to_string())?
        .map_err(|_| "Sign-in was cancelled".to_string())?;
    let _ = tauri_plugin_oauth::cancel(port);

    let parsed = url::Url::parse(&redirect).map_err(|e| e.to_string())?;
    let mut code = None;
    let mut state = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            _ => {}
        }
    }
    let code = code.ok_or("No authorization code returned")?;
    if state.as_deref() != Some(csrf.secret().as_str()) {
        return Err("State mismatch — possible CSRF, aborting".to_string());
    }

    let token = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http_client()?)
        .await
        .map_err(|e| e.to_string())?;

    let access_token = token.access_token().secret().to_string();
    let refresh_token = token.refresh_token().map(|r| r.secret().to_string());
    let expires_at = now_unix() + token.expires_in().map(|d| d.as_secs()).unwrap_or(3600);
    let (email, name) = fetch_userinfo(&base, &access_token).await;

    save_tokens(&StoredTokens {
        access_token,
        refresh_token,
        expires_at,
        email: email.clone(),
        name: name.clone(),
    })?;

    Ok(AuthStatus { signed_in: true, email, name })
}

#[tauri::command]
pub async fn auth_status() -> AuthStatus {
    let Some(tokens) = load_tokens() else {
        return AuthStatus::signed_out();
    };
    // Still valid (with a small skew buffer).
    if tokens.expires_at > now_unix() + 60 {
        return AuthStatus { signed_in: true, email: tokens.email, name: tokens.name };
    }
    // Expired — try to refresh, otherwise sign out.
    match tokens.refresh_token.clone() {
        Some(rt) => match refresh(rt, tokens).await {
            Ok(status) => status,
            Err(_) => {
                let _ = clear_tokens();
                AuthStatus::signed_out()
            }
        },
        None => {
            let _ = clear_tokens();
            AuthStatus::signed_out()
        }
    }
}

#[tauri::command]
pub async fn sign_out() -> Result<(), String> {
    clear_tokens()
}

/// A currently-valid access token for calling the backend, refreshing if needed.
/// Used by the GraphQL proxy so the token never leaves Rust.
pub(crate) async fn access_token() -> Option<String> {
    let tokens = load_tokens()?;
    if tokens.expires_at > now_unix() + 60 {
        return Some(tokens.access_token);
    }
    let refresh_token = tokens.refresh_token.clone()?;
    refresh(refresh_token, tokens).await.ok()?;
    load_tokens().map(|t| t.access_token)
}

async fn refresh(refresh_token: String, prev: StoredTokens) -> Result<AuthStatus, String> {
    let token = oauth_client()?
        .exchange_refresh_token(&RefreshToken::new(refresh_token))
        .request_async(&http_client()?)
        .await
        .map_err(|e| e.to_string())?;

    let access_token = token.access_token().secret().to_string();
    let refresh_token = token
        .refresh_token()
        .map(|r| r.secret().to_string())
        .or(prev.refresh_token);
    let expires_at = now_unix() + token.expires_in().map(|d| d.as_secs()).unwrap_or(3600);

    save_tokens(&StoredTokens {
        access_token,
        refresh_token,
        expires_at,
        email: prev.email.clone(),
        name: prev.name.clone(),
    })?;

    Ok(AuthStatus { signed_in: true, email: prev.email, name: prev.name })
}
