use serde_json::{Value, json};

use super::{Channel, ChannelError};

#[derive(Debug, Clone, Default)]
pub struct LastValue {
    key: String,
    value: Option<Value>,
}

impl LastValue {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: None,
        }
    }
}

impl Channel for LastValue {
    fn kind(&self) -> &'static str {
        "last_value"
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

        if values.len() != 1 {
            return Err(ChannelError::invalid_update(
                self.key(),
                "Can receive only one value per step. Use an Annotated key to handle multiple values.",
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

#[derive(Debug, Clone, Default)]
pub struct LastValueAfterFinish {
    key: String,
    value: Option<Value>,
    finished: bool,
}

impl LastValueAfterFinish {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: None,
            finished: false,
        }
    }
}

impl Channel for LastValueAfterFinish {
    fn kind(&self) -> &'static str {
        "last_value_after_finish"
    }

    fn key(&self) -> &str {
        &self.key
    }

    fn set_key(&mut self, key: String) {
        self.key = key;
    }

    fn get(&self) -> Result<Value, ChannelError> {
        if self.finished {
            self.value.clone().ok_or(ChannelError::EmptyChannel)
        } else {
            Err(ChannelError::EmptyChannel)
        }
    }

    fn update(&mut self, values: &[Value]) -> Result<bool, ChannelError> {
        if values.is_empty() {
            return Ok(false);
        }

        self.finished = false;
        self.value = values.last().cloned();
        Ok(true)
    }

    fn consume(&mut self) -> Result<bool, ChannelError> {
        if self.finished {
            self.finished = false;
            self.value = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn finish(&mut self) -> Result<bool, ChannelError> {
        if !self.finished && self.value.is_some() {
            self.finished = true;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn checkpoint(&self) -> Result<Option<Value>, ChannelError> {
        match &self.value {
            Some(value) => Ok(Some(json!({
                "value": value,
                "finished": self.finished,
            }))),
            None => Ok(None),
        }
    }

    fn from_checkpoint(
        &self,
        checkpoint: Option<&Value>,
    ) -> Result<Box<dyn Channel>, ChannelError> {
        let mut next = self.clone();
        next.value = None;
        next.finished = false;

        let Some(value) = checkpoint else {
            return Ok(Box::new(next));
        };

        let obj = value.as_object().ok_or_else(|| {
            ChannelError::invalid_checkpoint(self.key(), "expected object checkpoint")
        })?;

        let restored_value = obj
            .get("value")
            .ok_or_else(|| ChannelError::invalid_checkpoint(self.key(), "missing 'value'"))?
            .clone();

        let restored_finished = obj
            .get("finished")
            .and_then(Value::as_bool)
            .ok_or_else(|| ChannelError::invalid_checkpoint(self.key(), "missing 'finished'"))?;

        next.value = Some(restored_value);
        next.finished = restored_finished;

        Ok(Box::new(next))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Channel, LastValue, LastValueAfterFinish};

    #[test]
    fn last_value_enforces_single_update() {
        let mut channel = LastValue::new("x");
        let err = channel.update(&[json!(1), json!(2)]).unwrap_err();
        assert!(format!("{err}").contains("only one"));
    }

    #[test]
    fn last_value_after_finish_is_hidden_until_finish() {
        let mut channel = LastValueAfterFinish::new("x");
        channel.update(&[json!(42)]).unwrap();
        assert!(channel.get().is_err());
        channel.finish().unwrap();
        assert_eq!(channel.get().unwrap(), json!(42));
    }
}
