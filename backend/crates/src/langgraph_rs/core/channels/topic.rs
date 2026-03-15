use serde_json::Value;

use super::{Channel, ChannelError};

#[derive(Debug, Clone)]
pub struct Topic {
    key: String,
    values: Vec<Value>,
    accumulate: bool,
}

impl Topic {
    pub fn new(key: impl Into<String>, accumulate: bool) -> Self {
        Self {
            key: key.into(),
            values: Vec::new(),
            accumulate,
        }
    }
}

impl Channel for Topic {
    fn kind(&self) -> &'static str {
        "topic"
    }

    fn key(&self) -> &str {
        &self.key
    }

    fn set_key(&mut self, key: String) {
        self.key = key;
    }

    fn get(&self) -> Result<Value, ChannelError> {
        if self.values.is_empty() {
            Err(ChannelError::EmptyChannel)
        } else {
            Ok(Value::Array(self.values.clone()))
        }
    }

    fn update(&mut self, values: &[Value]) -> Result<bool, ChannelError> {
        let mut updated = false;

        if !self.accumulate {
            if !self.values.is_empty() {
                updated = true;
            }
            self.values.clear();
        }

        for value in values {
            match value {
                Value::Array(arr) => {
                    if !arr.is_empty() {
                        updated = true;
                        self.values.extend(arr.clone());
                    }
                }
                other => {
                    updated = true;
                    self.values.push(other.clone());
                }
            }
        }

        Ok(updated)
    }

    fn checkpoint(&self) -> Result<Option<Value>, ChannelError> {
        Ok(Some(Value::Array(self.values.clone())))
    }

    fn from_checkpoint(
        &self,
        checkpoint: Option<&Value>,
    ) -> Result<Box<dyn Channel>, ChannelError> {
        let mut next = self.clone();
        next.values.clear();

        let Some(value) = checkpoint else {
            return Ok(Box::new(next));
        };

        let arr = value.as_array().ok_or_else(|| {
            ChannelError::invalid_checkpoint(self.key(), "expected array checkpoint")
        })?;

        let decoded = if arr.len() == 2 && arr[0].is_boolean() {
            arr[1]
                .as_array()
                .ok_or_else(|| {
                    ChannelError::invalid_checkpoint(
                        self.key(),
                        "legacy tuple-like checkpoint must contain array values at index 1",
                    )
                })?
                .clone()
        } else {
            arr.clone()
        };

        next.values = decoded;
        Ok(Box::new(next))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Channel, Topic};

    #[test]
    fn topic_flattens_array_updates() {
        let mut channel = Topic::new("x", true);
        channel
            .update(&[json!(1), json!([2, 3]), json!(4)])
            .unwrap();

        assert_eq!(channel.get().unwrap(), json!([1, 2, 3, 4]));
    }

    #[test]
    fn restores_legacy_tuple_like_checkpoint_shape() {
        let channel = Topic::new("x", true);
        let restored = channel
            .from_checkpoint(Some(&json!([true, [1, 2, 3]])))
            .unwrap();

        assert_eq!(restored.get().unwrap(), json!([1, 2, 3]));
    }
}
