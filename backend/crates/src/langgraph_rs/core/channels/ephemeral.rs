use serde_json::Value;

use super::{Channel, ChannelError};

#[derive(Debug, Clone)]
pub struct EphemeralValue {
    key: String,
    value: Option<Value>,
    guard: bool,
}

impl EphemeralValue {
    pub fn new(key: impl Into<String>, guard: bool) -> Self {
        Self {
            key: key.into(),
            value: None,
            guard,
        }
    }
}

impl Channel for EphemeralValue {
    fn kind(&self) -> &'static str {
        "ephemeral_value"
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

        if self.guard && values.len() != 1 {
            return Err(ChannelError::invalid_update(
                self.key(),
                "EphemeralValue(guard=true) can receive only one value per step. Use guard=false if you want to store any one of multiple values.",
            ));
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

    use super::{Channel, EphemeralValue};

    #[test]
    fn clears_when_empty_update_arrives() {
        let mut channel = EphemeralValue::new("x", true);
        channel.update(&[json!("v")]).unwrap();
        assert!(channel.get().is_ok());
        channel.update(&[]).unwrap();
        assert!(channel.get().is_err());
    }
}
