#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::core::channels::{Channel, UntrackedValue};

    #[test]
    fn untracked_value_guard_and_non_persistence_parity() {
        let mut guarded = UntrackedValue::new("scratch", true);
        let err = guarded.update(&[json!(1), json!(2)]).unwrap_err();
        assert!(format!("{err}").contains("only one value per step"));

        let mut unguarded = UntrackedValue::new("scratch", false);
        unguarded.update(&[json!(1), json!(2)]).unwrap();
        assert_eq!(unguarded.get().unwrap(), json!(2));
        assert_eq!(unguarded.checkpoint().unwrap(), None);

        let restored = unguarded.from_checkpoint(Some(&json!(2))).unwrap();
        assert!(restored.get().is_err());
    }
}
