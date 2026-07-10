use tradstry_backend::service::ai::projector::{project, seed};

const PARAGRAPH_JSON: &str = r#"{"root":{"children":[{"type":"paragraph","children":[{"type":"text","text":"hello","format":0,"detail":0,"mode":"normal","style":"","version":1}],"direction":null,"format":"","indent":0,"version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#;

#[tokio::test]
async fn projector_round_trips_a_paragraph() {
    let update = seed(PARAGRAPH_JSON).await.unwrap().update;
    let out = project(&[update]).await.unwrap();
    assert!(out.contains("hello"), "projection lost the text: {out}");
}

#[tokio::test]
async fn projector_reports_failure_loudly() {
    let err = project(&[vec![0xff, 0xff]]).await;
    assert!(
        err.is_err(),
        "a corrupt update must fail, not return an empty document"
    );
}

#[tokio::test]
async fn project_with_no_updates_is_an_error() {
    let err = project(&[]).await;
    assert!(
        err.is_err(),
        "projecting zero updates must fail without spawning a subprocess"
    );
}

#[tokio::test]
async fn seed_rejects_malformed_json() {
    let err = seed("{}").await;
    assert!(err.is_err(), "seeding malformed JSON must fail");
}
