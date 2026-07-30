use tradstry_backend::routes::clerk_webhook::deleted_user_id;

#[test]
fn extracts_the_clerk_id_from_a_user_deleted_event() {
    let payload = serde_json::json!({
        "type": "user.deleted",
        "data": { "id": "user_abc123", "deleted": true }
    });
    assert_eq!(deleted_user_id(&payload), Some("user_abc123"));
}

#[test]
fn ignores_other_event_types() {
    let payload = serde_json::json!({
        "type": "user.updated",
        "data": { "id": "user_abc123" }
    });
    assert_eq!(deleted_user_id(&payload), None);
}

#[test]
fn ignores_a_deleted_event_with_no_id() {
    let payload = serde_json::json!({ "type": "user.deleted", "data": {} });
    assert_eq!(deleted_user_id(&payload), None);
}
