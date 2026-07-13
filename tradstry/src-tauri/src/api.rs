// Internal GraphQL transport. Domain commands (analytics, trades, …) build on
// this; it is NOT a Tauri command, so the webview can only reach the backend
// through the typed commands we expose — never a raw query. The Keychain access
// token is attached here and never leaves Rust.

use serde_json::Value;

/// Backend GraphQL endpoint. Override at build time with TRADSTRY_BACKEND_URL
/// (e.g. in src-tauri/.cargo/config.toml) to point at prod.
pub(crate) fn backend_url() -> &'static str {
    option_env!("TRADSTRY_BACKEND_URL").unwrap_or("http://localhost:7899/graphql")
}

/// Run a GraphQL query with the signed-in user's token and return `data`.
pub(crate) async fn graphql(query: &str, variables: Value) -> Result<Value, String> {
    let token = crate::auth::access_token().await.ok_or("Not signed in")?;

    let body = serde_json::json!({ "query": query, "variables": variables });

    let resp = reqwest::Client::new()
        .post(backend_url())
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let json: Value = resp.json().await.map_err(|e| e.to_string())?;

    if let Some(errors) = json.get("errors").filter(|e| !e.is_null()) {
        return Err(format!("GraphQL error: {errors}"));
    }
    if !status.is_success() {
        return Err(format!("Backend returned {status}"));
    }

    Ok(json.get("data").cloned().unwrap_or(Value::Null))
}
