use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use xxhash_rust::xxh3::xxh3_128;

pub type InterruptId = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Interrupt {
    pub id: InterruptId,
    pub value: Value,
}

impl Interrupt {
    pub fn new(value: Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            value,
        }
    }

    pub fn from_namespace(value: Value, namespace: &str) -> Self {
        Self {
            id: interrupt_id_from_namespace(namespace),
            value,
        }
    }

    pub fn from_namespace_parts(
        value: Value,
        namespace_parts: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Self {
        let namespace = namespace_parts
            .into_iter()
            .map(|part| part.as_ref().to_owned())
            .collect::<Vec<_>>()
            .join("|");
        Self::from_namespace(value, &namespace)
    }

    pub fn new_with_namespace(value: Value, namespace: Option<&str>) -> Self {
        if let Some(namespace) = namespace {
            return Self::from_namespace(value, namespace);
        }
        Self::new(value)
    }

    pub fn with_id(id: impl Into<InterruptId>, value: Value) -> Self {
        Self {
            id: id.into(),
            value,
        }
    }
}

pub fn interrupt_id_from_namespace(namespace: &str) -> InterruptId {
    format!("{:032x}", xxh3_128(namespace.as_bytes()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Interrupt, interrupt_id_from_namespace};

    #[test]
    fn same_namespace_produces_stable_interrupt_ids() {
        let first = Interrupt::from_namespace(json!("v"), "thread|node");
        let second = Interrupt::from_namespace(json!("v"), "thread|node");
        assert_eq!(first.id, second.id);
    }

    #[test]
    fn different_namespace_produces_different_interrupt_ids() {
        let first = Interrupt::from_namespace(json!("v"), "ns:a");
        let second = Interrupt::from_namespace(json!("v"), "ns:b");
        assert_ne!(first.id, second.id);
    }

    #[test]
    fn new_with_namespace_none_falls_back_to_uuid() {
        let interrupt = Interrupt::new_with_namespace(json!("v"), None);
        assert_eq!(interrupt.id.len(), 36);
        assert_eq!(interrupt.id.chars().filter(|ch| *ch == '-').count(), 4);
    }

    #[test]
    fn from_namespace_parts_matches_joined_namespace_hash() {
        let joined = interrupt_id_from_namespace("a|b");
        let interrupt = Interrupt::from_namespace_parts(json!("v"), vec!["a", "b"]);
        assert_eq!(interrupt.id, joined);
    }
}
