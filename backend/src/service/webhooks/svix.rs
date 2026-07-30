use anyhow::{Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

const TOLERANCE_SECONDS: i64 = 300;

type HmacSha256 = Hmac<Sha256>;

/// Verifies a Svix-signed webhook, the scheme Clerk uses.
///
/// `now_unix` is a parameter rather than read from the clock so the replay window is testable.
pub fn verify_svix_signature(
    secret: &str,
    svix_id: &str,
    svix_timestamp: &str,
    signature_header: &str,
    body: &[u8],
    now_unix: i64,
) -> Result<()> {
    let timestamp: i64 = svix_timestamp
        .parse()
        .map_err(|_| anyhow!("svix-timestamp is not an integer"))?;

    if (now_unix - timestamp).abs() > TOLERANCE_SECONDS {
        bail!("svix-timestamp outside the {TOLERANCE_SECONDS}s tolerance");
    }

    let key = STANDARD
        .decode(secret.strip_prefix("whsec_").unwrap_or(secret))
        .map_err(|_| anyhow!("webhook secret is not base64"))?;

    let mut mac =
        HmacSha256::new_from_slice(&key).map_err(|_| anyhow!("invalid webhook secret length"))?;
    mac.update(svix_id.as_bytes());
    mac.update(b".");
    mac.update(svix_timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);
    let expected = mac.finalize().into_bytes();

    let matched = signature_header
        .split(' ')
        .filter_map(|part| part.strip_prefix("v1,"))
        .filter_map(|encoded| STANDARD.decode(encoded).ok())
        .any(|candidate| candidate.ct_eq(&expected).into());

    if !matched {
        bail!("no svix signature matched");
    }

    Ok(())
}
