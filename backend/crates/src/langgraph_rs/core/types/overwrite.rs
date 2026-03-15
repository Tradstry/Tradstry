use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const OVERWRITE_MARKER: &str = "__overwrite__";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Overwrite {
    #[serde(rename = "__overwrite__")]
    pub value: Value,
}

impl Overwrite {
    pub fn new(value: Value) -> Self {
        Self { value }
    }

    pub fn into_value(self) -> Value {
        self.into()
    }
}

impl From<Overwrite> for Value {
    fn from(value: Overwrite) -> Self {
        let mut obj = Map::new();
        obj.insert(OVERWRITE_MARKER.to_owned(), value.value);
        Value::Object(obj)
    }
}

pub fn extract_overwrite_value(value: &Value) -> Option<Value> {
    match value {
        Value::Object(obj) if obj.len() == 1 => obj.get(OVERWRITE_MARKER).cloned(),
        _ => None,
    }
}

pub fn is_overwrite_value(value: &Value) -> bool {
    extract_overwrite_value(value).is_some()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Overwrite, extract_overwrite_value, is_overwrite_value};

    #[test]
    fn overwrite_value_round_trip_is_stable() {
        let overwrite = Overwrite::new(json!({"x": 1}));
        let value = overwrite.clone().into_value();

        assert!(is_overwrite_value(&value));
        assert_eq!(extract_overwrite_value(&value), Some(overwrite.value));
    }
}
