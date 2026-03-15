#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::core::channels::{AnyValue, Channel};

    #[test]
    fn any_value_parity_state_transitions() {
        let mut channel = AnyValue::new("messages");

        assert!(!channel.update(&[]).unwrap());

        assert!(channel.update(&[json!("first")]).unwrap());
        assert_eq!(channel.get().unwrap(), json!("first"));

        assert!(channel.update(&[json!("a"), json!("b")]).unwrap());
        assert_eq!(channel.get().unwrap(), json!("b"));

        assert!(channel.update(&[]).unwrap());
        assert!(channel.get().is_err());
    }
}
