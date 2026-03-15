use std::collections::{BTreeMap, BTreeSet};

use crate::langgraph_rs::core::{
    channels::{
        AnyValue, BinaryOperatorAggregate, BinaryOperatorFn, Channel, LastValue, Topic,
        UntrackedValue,
    },
    managed::{ManagedValueRef, ManagedValueRegistry},
    types::ChannelName,
};

use super::{GraphError, ManagedValueKind};

#[derive(Clone)]
pub enum StateFieldKind {
    Channel(Box<dyn Channel>),
    Managed(ManagedValueKind),
    CustomManaged(ManagedValueRef),
}

impl std::fmt::Debug for StateFieldKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Channel(channel) => f.debug_tuple("Channel").field(&channel.kind()).finish(),
            Self::Managed(kind) => f.debug_tuple("Managed").field(kind).finish(),
            Self::CustomManaged(_) => f.write_str("CustomManaged(<fn>)"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StateField {
    pub name: ChannelName,
    pub kind: StateFieldKind,
}

impl StateField {
    pub fn channel(name: impl Into<ChannelName>, mut channel: Box<dyn Channel>) -> Self {
        let name = name.into();
        channel.set_key(name.clone());
        Self {
            name,
            kind: StateFieldKind::Channel(channel),
        }
    }

    pub fn last_value(name: impl Into<ChannelName>) -> Self {
        let name = name.into();
        Self::channel(name.clone(), Box::new(LastValue::new(name)))
    }

    pub fn topic(name: impl Into<ChannelName>, accumulate: bool) -> Self {
        let name = name.into();
        Self::channel(name.clone(), Box::new(Topic::new(name, accumulate)))
    }

    pub fn any_value(name: impl Into<ChannelName>) -> Self {
        let name = name.into();
        Self::channel(name.clone(), Box::new(AnyValue::new(name)))
    }

    pub fn untracked_value(name: impl Into<ChannelName>, guard: bool) -> Self {
        let name = name.into();
        Self::channel(name.clone(), Box::new(UntrackedValue::new(name, guard)))
    }

    pub fn binary_operator(name: impl Into<ChannelName>, operator: BinaryOperatorFn) -> Self {
        let name = name.into();
        Self::channel(
            name.clone(),
            Box::new(BinaryOperatorAggregate::new(name, operator)),
        )
    }

    pub fn managed(name: impl Into<ChannelName>, kind: ManagedValueKind) -> Self {
        Self {
            name: name.into(),
            kind: StateFieldKind::Managed(kind),
        }
    }

    pub fn custom_managed(name: impl Into<ChannelName>, managed: ManagedValueRef) -> Self {
        Self {
            name: name.into(),
            kind: StateFieldKind::CustomManaged(managed),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct StateSchema {
    fields: BTreeMap<ChannelName, StateFieldKind>,
}

impl StateSchema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_field(&mut self, field: StateField) -> Result<&mut Self, GraphError> {
        match self.fields.get(&field.name) {
            None => {
                self.fields.insert(field.name, field.kind);
                Ok(self)
            }
            Some(existing) => {
                let existing_kind = Self::kind_label(existing);
                let next_kind = Self::kind_label(&field.kind);
                Err(GraphError::InvalidSchemaField {
                    field: field.name,
                    message: format!(
                        "field already exists with kind '{existing_kind}' and cannot be redefined as '{next_kind}'"
                    ),
                })
            }
        }
    }

    pub fn with_field(mut self, field: StateField) -> Result<Self, GraphError> {
        self.add_field(field)?;
        Ok(self)
    }

    pub fn with_last_value(mut self, name: impl Into<ChannelName>) -> Result<Self, GraphError> {
        self.add_field(StateField::last_value(name))?;
        Ok(self)
    }

    pub fn with_topic(
        mut self,
        name: impl Into<ChannelName>,
        accumulate: bool,
    ) -> Result<Self, GraphError> {
        self.add_field(StateField::topic(name, accumulate))?;
        Ok(self)
    }

    pub fn with_any_value(mut self, name: impl Into<ChannelName>) -> Result<Self, GraphError> {
        self.add_field(StateField::any_value(name))?;
        Ok(self)
    }

    pub fn with_untracked_value(
        mut self,
        name: impl Into<ChannelName>,
        guard: bool,
    ) -> Result<Self, GraphError> {
        self.add_field(StateField::untracked_value(name, guard))?;
        Ok(self)
    }

    pub fn with_binary_operator(
        mut self,
        name: impl Into<ChannelName>,
        operator: BinaryOperatorFn,
    ) -> Result<Self, GraphError> {
        self.add_field(StateField::binary_operator(name, operator))?;
        Ok(self)
    }

    pub fn with_managed(
        mut self,
        name: impl Into<ChannelName>,
        kind: ManagedValueKind,
    ) -> Result<Self, GraphError> {
        self.add_field(StateField::managed(name, kind))?;
        Ok(self)
    }

    pub fn with_custom_managed(
        mut self,
        name: impl Into<ChannelName>,
        managed: ManagedValueRef,
    ) -> Result<Self, GraphError> {
        self.add_field(StateField::custom_managed(name, managed))?;
        Ok(self)
    }

    pub fn has_field(&self, name: &str) -> bool {
        self.fields.contains_key(name)
    }

    pub fn fields(&self) -> &BTreeMap<ChannelName, StateFieldKind> {
        &self.fields
    }

    pub fn without_managed(&self) -> Self {
        let mut schema = Self::new();
        for (name, kind) in &self.fields {
            if let StateFieldKind::Channel(channel) = kind {
                let mut cloned = channel.clone();
                cloned.set_key(name.clone());
                schema
                    .fields
                    .insert(name.clone(), StateFieldKind::Channel(cloned));
            }
        }
        schema
    }

    pub fn field_names(&self) -> Vec<String> {
        self.fields.keys().cloned().collect()
    }

    pub fn channel_names(&self) -> Vec<String> {
        self.fields
            .iter()
            .filter_map(|(name, kind)| match kind {
                StateFieldKind::Channel(_) => Some(name.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn managed_names(&self) -> Vec<String> {
        self.fields
            .iter()
            .filter_map(|(name, kind)| match kind {
                StateFieldKind::Channel(_) => None,
                _ => Some(name.clone()),
            })
            .collect()
    }

    pub fn readable_names(&self) -> Vec<String> {
        self.fields.keys().cloned().collect()
    }

    pub fn channels(&self) -> BTreeMap<ChannelName, Box<dyn Channel>> {
        self.fields
            .iter()
            .filter_map(|(name, kind)| match kind {
                StateFieldKind::Channel(channel) => {
                    let mut cloned = channel.clone();
                    cloned.set_key(name.clone());
                    Some((name.clone(), cloned))
                }
                _ => None,
            })
            .collect()
    }

    pub fn managed_values(&self) -> ManagedValueRegistry {
        self.fields
            .iter()
            .filter_map(|(name, kind)| match kind {
                StateFieldKind::Managed(kind) => Some((name.clone(), kind.to_managed_value())),
                StateFieldKind::CustomManaged(managed) => Some((name.clone(), managed.clone())),
                StateFieldKind::Channel(_) => None,
            })
            .collect()
    }

    pub fn validate_no_managed(&self, schema_label: &str) -> Result<(), GraphError> {
        let managed = self
            .fields
            .iter()
            .filter_map(|(name, kind)| match kind {
                StateFieldKind::Channel(_) => None,
                _ => Some(name.clone()),
            })
            .collect::<BTreeSet<_>>();
        if managed.is_empty() {
            return Ok(());
        }
        let field = managed.iter().next().cloned().unwrap_or_default();
        Err(GraphError::InvalidManagedField {
            field,
            message: format!(
                "{schema_label} schema cannot contain managed fields: {}",
                managed.into_iter().collect::<Vec<_>>().join(", ")
            ),
        })
    }

    fn kind_label(kind: &StateFieldKind) -> String {
        match kind {
            StateFieldKind::Channel(channel) => channel.kind().to_owned(),
            StateFieldKind::Managed(kind) => format!("managed::{kind:?}"),
            StateFieldKind::CustomManaged(_) => "managed::custom".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::core::{
        channels::{BinaryOperatorAggregate, Channel},
        graph::{ManagedValueKind, StateField, StateSchema},
    };

    #[test]
    fn collects_channels_and_managed_values() {
        let schema = StateSchema::new()
            .with_last_value("input")
            .unwrap()
            .with_managed("remaining_steps", ManagedValueKind::RemainingSteps)
            .unwrap();

        let channels = schema.channels();
        let managed = schema.managed_values();

        assert!(channels.contains_key("input"));
        assert!(managed.contains_key("remaining_steps"));
    }

    #[test]
    fn rejects_duplicate_field_redefinitions() {
        let mut schema = StateSchema::new();
        schema.add_field(StateField::last_value("x")).unwrap();
        let err = schema.add_field(StateField::any_value("x")).unwrap_err();
        assert!(format!("{err}").contains("cannot be redefined"));
    }

    #[test]
    fn managed_fields_are_disallowed_in_io_schema() {
        let schema = StateSchema::new()
            .with_last_value("x")
            .unwrap()
            .with_managed("is_last_step", ManagedValueKind::IsLastStep)
            .unwrap();
        let err = schema.validate_no_managed("input").unwrap_err();
        assert!(format!("{err}").contains("cannot contain managed fields"));
    }

    #[test]
    fn binary_operator_field_supports_reducer_channels() {
        let schema = StateSchema::new()
            .with_field(StateField::binary_operator(
                "sum",
                std::sync::Arc::new(|left, right| {
                    let value = left.as_i64().unwrap_or(0) + right.as_i64().unwrap_or(0);
                    Ok(json!(value))
                }),
            ))
            .unwrap();

        let mut channel = schema
            .channels()
            .remove("sum")
            .expect("sum channel should exist");
        channel.update(&[json!(1), json!(2), json!(3)]).unwrap();
        assert_eq!(channel.get().unwrap(), json!(6));

        let is_binop = matches!(
            schema.fields().get("sum"),
            Some(super::StateFieldKind::Channel(channel))
            if channel.kind() == BinaryOperatorAggregate::add_numeric("sum").kind()
        );
        assert!(is_binop);
    }
}
