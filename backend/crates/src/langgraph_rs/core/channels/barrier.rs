use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::{Channel, ChannelError};

#[derive(Debug, Clone)]
pub struct NamedBarrierValue {
    key: String,
    names: BTreeSet<String>,
    seen: BTreeSet<String>,
}

impl NamedBarrierValue {
    pub fn new(key: impl Into<String>, names: impl IntoIterator<Item = String>) -> Self {
        Self {
            key: key.into(),
            names: names.into_iter().collect(),
            seen: BTreeSet::new(),
        }
    }

    fn parse_name(&self, value: &Value) -> Result<String, ChannelError> {
        value
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| ChannelError::invalid_update(self.key(), "expected string value"))
    }
}

impl Channel for NamedBarrierValue {
    fn kind(&self) -> &'static str {
        "named_barrier_value"
    }

    fn key(&self) -> &str {
        &self.key
    }

    fn set_key(&mut self, key: String) {
        self.key = key;
    }

    fn get(&self) -> Result<Value, ChannelError> {
        if self.seen == self.names {
            Ok(Value::Null)
        } else {
            Err(ChannelError::EmptyChannel)
        }
    }

    fn update(&mut self, values: &[Value]) -> Result<bool, ChannelError> {
        let mut updated = false;

        for value in values {
            let name = self.parse_name(value)?;
            if !self.names.contains(&name) {
                return Err(ChannelError::invalid_update(
                    self.key(),
                    format!("value '{name}' not in barrier set"),
                ));
            }
            if self.seen.insert(name) {
                updated = true;
            }
        }

        Ok(updated)
    }

    fn consume(&mut self) -> Result<bool, ChannelError> {
        if self.seen == self.names {
            self.seen.clear();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn checkpoint(&self) -> Result<Option<Value>, ChannelError> {
        Ok(Some(Value::Array(
            self.seen.iter().cloned().map(Value::String).collect(),
        )))
    }

    fn restore_from_checkpoint(
        &self,
        checkpoint: Option<&Value>,
    ) -> Result<Box<dyn Channel>, ChannelError> {
        let mut next = self.clone();
        next.seen.clear();

        let Some(value) = checkpoint else {
            return Ok(Box::new(next));
        };

        let arr = value.as_array().ok_or_else(|| {
            ChannelError::invalid_checkpoint(self.key(), "expected array checkpoint")
        })?;

        for item in arr {
            let Some(name) = item.as_str() else {
                return Err(ChannelError::invalid_checkpoint(
                    self.key(),
                    "checkpoint array must contain only strings",
                ));
            };
            next.seen.insert(name.to_owned());
        }

        Ok(Box::new(next))
    }
}

#[derive(Debug, Clone)]
pub struct NamedBarrierValueAfterFinish {
    key: String,
    names: BTreeSet<String>,
    seen: BTreeSet<String>,
    finished: bool,
}

impl NamedBarrierValueAfterFinish {
    pub fn new(key: impl Into<String>, names: impl IntoIterator<Item = String>) -> Self {
        Self {
            key: key.into(),
            names: names.into_iter().collect(),
            seen: BTreeSet::new(),
            finished: false,
        }
    }

    fn parse_name(&self, value: &Value) -> Result<String, ChannelError> {
        value
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| ChannelError::invalid_update(self.key(), "expected string value"))
    }
}

impl Channel for NamedBarrierValueAfterFinish {
    fn kind(&self) -> &'static str {
        "named_barrier_value_after_finish"
    }

    fn key(&self) -> &str {
        &self.key
    }

    fn set_key(&mut self, key: String) {
        self.key = key;
    }

    fn get(&self) -> Result<Value, ChannelError> {
        if self.finished && self.seen == self.names {
            Ok(Value::Null)
        } else {
            Err(ChannelError::EmptyChannel)
        }
    }

    fn update(&mut self, values: &[Value]) -> Result<bool, ChannelError> {
        let mut updated = false;

        for value in values {
            let name = self.parse_name(value)?;
            if !self.names.contains(&name) {
                return Err(ChannelError::invalid_update(
                    self.key(),
                    format!("value '{name}' not in barrier set"),
                ));
            }
            if self.seen.insert(name) {
                updated = true;
            }
        }

        Ok(updated)
    }

    fn consume(&mut self) -> Result<bool, ChannelError> {
        if self.finished && self.seen == self.names {
            self.finished = false;
            self.seen.clear();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn finish(&mut self) -> Result<bool, ChannelError> {
        if !self.finished && self.seen == self.names {
            self.finished = true;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn checkpoint(&self) -> Result<Option<Value>, ChannelError> {
        Ok(Some(json!({
            "seen": self.seen.iter().cloned().collect::<Vec<_>>(),
            "finished": self.finished,
        })))
    }

    fn restore_from_checkpoint(
        &self,
        checkpoint: Option<&Value>,
    ) -> Result<Box<dyn Channel>, ChannelError> {
        let mut next = self.clone();
        next.seen.clear();
        next.finished = false;

        let Some(value) = checkpoint else {
            return Ok(Box::new(next));
        };

        let obj = value.as_object().ok_or_else(|| {
            ChannelError::invalid_checkpoint(self.key(), "expected object checkpoint")
        })?;

        let seen = obj
            .get("seen")
            .and_then(Value::as_array)
            .ok_or_else(|| ChannelError::invalid_checkpoint(self.key(), "missing 'seen'"))?;

        for item in seen {
            let Some(name) = item.as_str() else {
                return Err(ChannelError::invalid_checkpoint(
                    self.key(),
                    "'seen' must contain only strings",
                ));
            };
            next.seen.insert(name.to_owned());
        }

        next.finished = obj
            .get("finished")
            .and_then(Value::as_bool)
            .ok_or_else(|| ChannelError::invalid_checkpoint(self.key(), "missing 'finished'"))?;

        Ok(Box::new(next))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Channel, NamedBarrierValue, NamedBarrierValueAfterFinish};

    #[test]
    fn barrier_waits_for_all_names() {
        let mut barrier = NamedBarrierValue::new("join", ["a".to_owned(), "b".to_owned()]);

        barrier.update(&[json!("a")]).unwrap();
        assert!(barrier.get().is_err());

        barrier.update(&[json!("b")]).unwrap();
        assert_eq!(barrier.get().unwrap(), serde_json::Value::Null);
    }

    #[test]
    fn barrier_after_finish_needs_finish_signal() {
        let mut barrier =
            NamedBarrierValueAfterFinish::new("join", ["a".to_owned(), "b".to_owned()]);

        barrier.update(&[json!("a"), json!("b")]).unwrap();
        assert!(barrier.get().is_err());

        barrier.finish().unwrap();
        assert_eq!(barrier.get().unwrap(), serde_json::Value::Null);
    }
}
