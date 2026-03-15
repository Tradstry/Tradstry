use serde_json::Value;

use super::{Channel, ChannelError};

#[derive(Debug, Clone)]
pub struct UntrackedValue {
    key: String,
    value: Option<Value>,
    guard: bool,
}

impl UntrackedValue {
    pub fn new(key: impl Into<String>, guard: bool) -> Self {
        Self {
            key: key.into(),
            value: None,
            guard,
        }
    }
}

impl Channel for UntrackedValue {
    fn kind(&self) -> &'static str {
        "untracked_value"
    }

    fn key(&self) -> &str {
        &self.key
    }

    fn set_key(&mut self, key: String) {
        self.key = key;
    }

    fn get(&self) -> Result<Value, ChannelError> {
        self.value.clone().ok_or(ChannelError::EmptyChannel)
    }

    fn update(&mut self, values: &[Value]) -> Result<bool, ChannelError> {
        if values.is_empty() {
            return Ok(false);
        }
        if self.guard && values.len() != 1 {
            return Err(ChannelError::invalid_update(
                self.key(),
                "UntrackedValue(guard=true) can receive only one value per step. Use guard=false if you want to store any one of multiple values.",
            ));
        }

        self.value = values.last().cloned();
        Ok(true)
    }

    fn checkpoint(&self) -> Result<Option<Value>, ChannelError> {
        Ok(None)
    }

    fn from_checkpoint(
        &self,
        _checkpoint: Option<&Value>,
    ) -> Result<Box<dyn Channel>, ChannelError> {
        let mut next = self.clone();
        next.value = None;
        Ok(Box::new(next))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Channel, UntrackedValue};

    #[test]
    fn guarded_mode_rejects_multiple_values() {
        let mut channel = UntrackedValue::new("scratch", true);
        let err = channel.update(&[json!(1), json!(2)]).unwrap_err();
        assert!(format!("{err}").contains("only one value per step"));
    }

    #[test]
    fn unguarded_mode_keeps_last_value() {
        let mut channel = UntrackedValue::new("scratch", false);
        channel.update(&[json!(1), json!(2)]).unwrap();
        assert_eq!(channel.get().unwrap(), json!(2));
    }

    #[test]
    fn does_not_persist_checkpoint_value() {
        let mut channel = UntrackedValue::new("scratch", true);
        channel.update(&[json!("secret")]).unwrap();

        assert_eq!(channel.checkpoint().unwrap(), None);

        let restored = channel.from_checkpoint(Some(&json!("secret"))).unwrap();
        assert!(restored.get().is_err());
    }

    #[test]
    fn clone_preserves_in_memory_value() {
        let mut channel = UntrackedValue::new("scratch", true);
        channel.update(&[json!("secret")]).unwrap();

        let cloned = channel.clone();
        assert_eq!(cloned.get().unwrap(), json!("secret"));
    }
}
