#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::langgraph_rs::{
        checkpoint::base::{
            CHECKPOINT_FORMAT_VERSION, CheckpointCompatibilityPolicy, CheckpointConfig,
            CheckpointMetadata, CheckpointWireFormat, create_checkpoint_with_config,
            deserialize_checkpoint_json, empty_checkpoint, get_serializable_checkpoint_metadata,
        },
        core::channels::{Channel, LastValue},
    };

    #[test]
    fn checkpoint_parity_projects_python_write_format_and_normalizes_on_read() {
        let config = CheckpointConfig::new("parity-checkpoint-thread").with_compatibility(
            CheckpointCompatibilityPolicy {
                write_format: CheckpointWireFormat::PythonV4,
                ..Default::default()
            },
        );

        let mut previous = empty_checkpoint();
        previous.channel_versions.insert("state".to_owned(), 7);

        let mut state = LastValue::new("state");
        state.update(&[json!({"k": "v"})]).unwrap();
        let mut channels = BTreeMap::<String, Box<dyn Channel>>::new();
        channels.insert("state".to_owned(), Box::new(state));

        let projected =
            create_checkpoint_with_config(&previous, Some(&channels), 3, None, Some(&config))
                .unwrap();
        assert_eq!(projected.v, CheckpointWireFormat::PythonV4.version());

        let serialized = serde_json::to_string(&projected).unwrap();
        let normalized =
            deserialize_checkpoint_json(&serialized, &CheckpointConfig::new("reader")).unwrap();

        assert_eq!(normalized.v, CHECKPOINT_FORMAT_VERSION);
        assert_eq!(
            normalized.channel_values.get("state"),
            Some(&json!({"k": "v"}))
        );
        assert_eq!(normalized.channel_versions.get("state"), Some(&7));
    }

    #[test]
    fn checkpoint_parity_metadata_merge_is_fill_only_and_scalar_sanitized() {
        let config = CheckpointConfig::new("parity-meta-thread")
            .with_metadata(BTreeMap::from([
                ("user".to_owned(), json!("alice\u{0000}")),
                ("existing".to_owned(), json!("override-me")),
                ("checkpoint_id".to_owned(), json!("skip")),
            ]))
            .with_configurable(BTreeMap::from([
                ("attempt".to_owned(), json!(2)),
                ("non_scalar".to_owned(), json!({"x": 1})),
                ("__private".to_owned(), json!("ignore")),
            ]));

        let mut metadata = CheckpointMetadata::default();
        metadata.extra.insert("existing".to_owned(), json!("kept"));
        metadata.extra.insert("writes".to_owned(), json!({"a": 1}));

        let serializable = get_serializable_checkpoint_metadata(&config, &metadata);

        assert_eq!(serializable.extra.get("existing"), Some(&json!("kept")));
        assert_eq!(serializable.extra.get("user"), Some(&json!("alice")));
        assert_eq!(serializable.extra.get("attempt"), Some(&json!(2)));
        assert!(!serializable.extra.contains_key("writes"));
        assert!(!serializable.extra.contains_key("checkpoint_id"));
        assert!(!serializable.extra.contains_key("non_scalar"));
        assert!(!serializable.extra.contains_key("__private"));
    }
}
