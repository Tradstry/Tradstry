use std::{fmt, sync::Arc};

use serde_json::{Number, Value};

use crate::langgraph_rs::core::types::extract_overwrite_value;

use super::{Channel, ChannelError};

pub type BinaryOperatorFn =
    Arc<dyn Fn(&Value, &Value) -> Result<Value, ChannelError> + Send + Sync>;

#[derive(Clone)]
pub struct BinaryOperatorAggregate {
    key: String,
    value: Option<Value>,
    operator: BinaryOperatorFn,
    operator_name: String,
}

impl fmt::Debug for BinaryOperatorAggregate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BinaryOperatorAggregate")
            .field("key", &self.key)
            .field("value", &self.value)
            .field("operator_name", &self.operator_name)
            .finish()
    }
}

impl BinaryOperatorAggregate {
    pub fn new(key: impl Into<String>, operator: BinaryOperatorFn) -> Self {
        Self::with_operator_name(key, "custom", operator)
    }

    pub fn with_operator_name(
        key: impl Into<String>,
        operator_name: impl Into<String>,
        operator: BinaryOperatorFn,
    ) -> Self {
        Self {
            key: key.into(),
            value: None,
            operator,
            operator_name: operator_name.into(),
        }
    }

    pub fn add_numeric(key: impl Into<String>) -> Self {
        Self::with_operator_name(
            key,
            "add_numeric",
            Arc::new(|left, right| {
                if let (Some(l), Some(r)) = (left.as_i64(), right.as_i64()) {
                    return Ok(Value::Number(Number::from(l.saturating_add(r))));
                }

                if let (Some(l), Some(r)) = (left.as_u64(), right.as_u64()) {
                    return Ok(Value::Number(Number::from(l.saturating_add(r))));
                }

                if let (Some(l), Some(r)) = (left.as_f64(), right.as_f64()) {
                    if let Some(number) = Number::from_f64(l + r) {
                        return Ok(Value::Number(number));
                    }
                }

                Err(ChannelError::invalid_update(
                    "binary_operator_aggregate",
                    "add_numeric operator requires numeric values",
                ))
            }),
        )
    }
}

impl Channel for BinaryOperatorAggregate {
    fn kind(&self) -> &'static str {
        "binary_operator_aggregate"
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

        let mut updated = false;
        let mut overwrite_seen = false;

        for next in values {
            if let Some(overwrite_value) = extract_overwrite_value(next) {
                if overwrite_seen {
                    return Err(ChannelError::invalid_update(
                        self.key(),
                        "Can receive only one Overwrite value per super-step.",
                    ));
                }
                self.value = Some(overwrite_value);
                overwrite_seen = true;
                updated = true;
                continue;
            }

            if overwrite_seen {
                continue;
            }

            match self.value.take() {
                Some(current) => {
                    let combined = (self.operator)(&current, next)?;
                    self.value = Some(combined);
                }
                None => {
                    self.value = Some(next.clone());
                }
            }
            updated = true;
        }

        Ok(updated)
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
    use std::sync::Arc;

    use serde_json::json;

    use crate::langgraph_rs::core::types::Overwrite;

    use super::{BinaryOperatorAggregate, Channel};

    #[test]
    fn add_numeric_reduces_values() {
        let mut channel = BinaryOperatorAggregate::add_numeric("sum");
        channel.update(&[json!(1), json!(2), json!(3)]).unwrap();
        assert_eq!(channel.get().unwrap(), json!(6));
    }

    #[test]
    fn custom_operator_supports_list_append_style_reducer() {
        let append = Arc::new(|left: &serde_json::Value, right: &serde_json::Value| {
            let mut arr = left.as_array().cloned().ok_or_else(|| {
                super::ChannelError::invalid_update("items", "left value must be an array")
            })?;
            arr.push(right.clone());
            Ok(serde_json::Value::Array(arr))
        });

        let mut channel = BinaryOperatorAggregate::new("items", append);
        channel
            .update(&[json!([1]), json!(2), json!(3), json!(4)])
            .unwrap();
        assert_eq!(channel.get().unwrap(), json!([1, 2, 3, 4]));
    }

    #[test]
    fn overwrite_value_replaces_previous_and_ignores_following_values() {
        let mut channel = BinaryOperatorAggregate::add_numeric("sum");
        channel.update(&[json!(1), json!(2)]).unwrap();
        channel
            .update(&[Overwrite::new(json!(5)).into(), json!(100)])
            .unwrap();

        assert_eq!(channel.get().unwrap(), json!(5));
    }

    #[test]
    fn dict_form_overwrite_is_supported() {
        let mut channel = BinaryOperatorAggregate::add_numeric("sum");
        channel
            .update(&[json!(1), json!({"__overwrite__": 9}), json!(100)])
            .unwrap();

        assert_eq!(channel.get().unwrap(), json!(9));
    }

    #[test]
    fn rejects_multiple_overwrites_in_single_step() {
        let mut channel = BinaryOperatorAggregate::add_numeric("sum");
        let err = channel
            .update(&[Overwrite::new(json!(1)).into(), json!({"__overwrite__": 2})])
            .unwrap_err();

        assert!(format!("{err}").contains("only one Overwrite"));
    }

    #[test]
    fn empty_update_does_not_change_value() {
        let mut channel = BinaryOperatorAggregate::add_numeric("sum");
        assert!(!channel.update(&[]).unwrap());
    }
}
