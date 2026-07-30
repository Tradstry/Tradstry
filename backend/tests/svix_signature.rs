use tradstry_backend::service::webhooks::svix::verify_svix_signature;

const SECRET: &str = "whsec_dHJhZHN0cnktdGVzdC1zZWNyZXQtMDEyMzQ1Njc4OWFi";
const SVIX_ID: &str = "msg_2abcXYZ";
const TIMESTAMP: &str = "1753900000";
const BODY: &[u8] = br#"{"type":"user.deleted","data":{"id":"user_abc123","deleted":true}}"#;
const SIGNATURE: &str = "v1,2wnF6+g3CrpXlXMQ5el+NdOt8arXHCbJ5XeTAF4DQXU=";

const NOW: i64 = 1_753_900_060;

#[test]
fn accepts_a_valid_signature() {
    verify_svix_signature(SECRET, SVIX_ID, TIMESTAMP, SIGNATURE, BODY, NOW)
        .expect("valid signature should verify");
}

#[test]
fn accepts_when_one_of_several_signatures_matches() {
    let header = format!("v1,ZmFrZXNpZ25hdHVyZXZhbHVlMDAwMDAwMDAwMDAwMDA= {SIGNATURE}");
    verify_svix_signature(SECRET, SVIX_ID, TIMESTAMP, &header, BODY, NOW)
        .expect("should accept when any signature matches");
}

#[test]
fn rejects_a_tampered_body() {
    let tampered = br#"{"type":"user.deleted","data":{"id":"user_OTHER","deleted":true}}"#;
    assert!(verify_svix_signature(SECRET, SVIX_ID, TIMESTAMP, SIGNATURE, tampered, NOW).is_err());
}

#[test]
fn rejects_a_wrong_secret() {
    let other = "whsec_b3RoZXItc2VjcmV0LTAxMjM0NTY3ODlhYmNkZWY=";
    assert!(verify_svix_signature(other, SVIX_ID, TIMESTAMP, SIGNATURE, BODY, NOW).is_err());
}

#[test]
fn rejects_a_stale_timestamp() {
    let much_later = 1_753_900_000 + 3_600;
    assert!(
        verify_svix_signature(SECRET, SVIX_ID, TIMESTAMP, SIGNATURE, BODY, much_later).is_err(),
        "a replayed webhook an hour old must be rejected"
    );
}

#[test]
fn rejects_a_future_timestamp() {
    let long_before = 1_753_900_000 - 3_600;
    assert!(
        verify_svix_signature(SECRET, SVIX_ID, TIMESTAMP, SIGNATURE, BODY, long_before).is_err()
    );
}

#[test]
fn rejects_a_malformed_header() {
    assert!(verify_svix_signature(SECRET, SVIX_ID, TIMESTAMP, "garbage", BODY, NOW).is_err());
}
