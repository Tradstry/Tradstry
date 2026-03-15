use serde_json::Value;

use super::{Channel, ChannelError};

#[derive(Debug, Clone, Default)]
pub struct AnyValue {
    key: String,
    value: Option<Value>,
}

impl AnyValue {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: None,
        }
    }
}

impl Channel for AnyValue {
    fn kind(&self) -> &'static str {
        "any_value"
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
            if self.value.is_some() {
                self.value = None;
                return Ok(true);
            }
            return Ok(false);
        }

        self.value = values.last().cloned();
        Ok(true)
    }

    fn from_checkpoint(
        &self,
        checkpoint: Option<&Value>,
    ) -> Result<Box<dyn Channel>, ChannelError> {
        let mut next = self.clone();
        next.value = checkpoint.cloned();
        Ok(Box::new(next))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{AnyValue, Channel};

    #[test]
    fn empty_update_clears_existing_value() {
        let mut channel = AnyValue::new("x");
        assert!(!channel.update(&[]).unwrap());

        assert!(channel.update(&[json!(1)]).unwrap());
        assert_eq!(channel.get().unwrap(), json!(1));

        assert!(channel.update(&[]).unwrap());
        assert!(channel.get().is_err());
    }

    #[test]
    fn update_uses_last_value() {
        let mut channel = AnyValue::new("x");
        channel.update(&[json!(1), json!(2)]).unwrap();
        assert_eq!(channel.get().unwrap(), json!(2));
    }

    #[test]
    fn checkpoint_roundtrip_restores_value() {
        let mut channel = AnyValue::new("x");
        channel.update(&[json!({"k": "v"})]).unwrap();

        let checkpoint = channel.checkpoint().unwrap();
        let restored = channel.from_checkpoint(checkpoint.as_ref()).unwrap();

        assert_eq!(restored.get().unwrap(), json!({"k": "v"}));
    }
}
