use tradstry_backend::routes::notebook_media::{media_key, verify_hash};

#[test]
fn media_key_is_hash_addressed() {
    assert_eq!(media_key("user-1", "abc"), "notebook/user-1/media/abc");
}

#[test]
fn verify_hash_accepts_matching_and_rejects_mismatch() {
    // sha256("abc")
    let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    assert!(verify_hash(b"abc", abc).is_ok());
    assert!(verify_hash(b"abc", "0000").is_err());
}
