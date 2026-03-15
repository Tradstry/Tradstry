#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::langgraph_rs::checkpoint::base::{
        CheckpointConfig, CheckpointMetadata, get_serializable_checkpoint_metadata,
    };

    #[test]
    fn serializable_metadata_filters_excluded_private_and_non_scalar_keys() {
        let config = CheckpointConfig::new("parity-serde-allowlist")
            .with_metadata(BTreeMap::from([
                ("allowed".to_owned(), json!("ok")),
                ("checkpoint_id".to_owned(), json!("drop")),
                ("thread_id".to_owned(), json!("drop")),
                ("parent_config".to_owned(), json!("drop")),
            ]))
            .with_configurable(BTreeMap::from([
                ("attempt".to_owned(), json!(3)),
                ("__private".to_owned(), json!("drop")),
                ("nested".to_owned(), json!({"x": 1})),
            ]));

        let mut metadata = CheckpointMetadata::default();
        metadata.extra.insert("writes".to_owned(), json!({"x": 1}));
        metadata
            .extra
            .insert("user".to_owned(), json!("alice\u{0000}"));

        let serializable = get_serializable_checkpoint_metadata(&config, &metadata);

        assert_eq!(serializable.extra.get("allowed"), Some(&json!("ok")));
        assert_eq!(serializable.extra.get("attempt"), Some(&json!(3)));
        assert_eq!(serializable.extra.get("user"), Some(&json!("alice")));
        assert!(!serializable.extra.contains_key("writes"));
        assert!(!serializable.extra.contains_key("__private"));
        assert!(!serializable.extra.contains_key("nested"));
        assert!(!serializable.extra.contains_key("checkpoint_id"));
        assert!(!serializable.extra.contains_key("thread_id"));
    }
}
