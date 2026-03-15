use std::{collections::BTreeMap, sync::Arc};

use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagedValueContext {
    pub step: u64,
    pub stop: u64,
}

impl ManagedValueContext {
    pub fn new(step: u64, stop: u64) -> Self {
        Self { step, stop }
    }
}

pub trait ManagedValue: Send + Sync {
    fn get(&self, ctx: ManagedValueContext) -> Value;
}

pub type ManagedValueRef = Arc<dyn ManagedValue>;
pub type ManagedValueRegistry = BTreeMap<String, ManagedValueRef>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltInManagedValue {
    IsLastStep,
    RemainingSteps,
}

impl BuiltInManagedValue {
    pub fn key(self) -> &'static str {
        match self {
            Self::IsLastStep => "is_last_step",
            Self::RemainingSteps => "remaining_steps",
        }
    }
}

impl ManagedValue for BuiltInManagedValue {
    fn get(&self, ctx: ManagedValueContext) -> Value {
        match self {
            Self::IsLastStep => json!(ctx.stop > 0 && ctx.step.saturating_add(1) == ctx.stop),
            Self::RemainingSteps => json!(ctx.stop.saturating_sub(ctx.step)),
        }
    }
}

#[derive(Clone)]
pub struct FnManagedValue {
    resolver: Arc<dyn Fn(ManagedValueContext) -> Value + Send + Sync>,
}

impl std::fmt::Debug for FnManagedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FnManagedValue").finish_non_exhaustive()
    }
}

impl FnManagedValue {
    pub fn new<F>(resolver: F) -> Self
    where
        F: Fn(ManagedValueContext) -> Value + Send + Sync + 'static,
    {
        Self {
            resolver: Arc::new(resolver),
        }
    }
}

impl ManagedValue for FnManagedValue {
    fn get(&self, ctx: ManagedValueContext) -> Value {
        (self.resolver)(ctx)
    }
}

pub fn built_in_managed_value(name: &str) -> Option<ManagedValueRef> {
    match name {
        "is_last_step" => Some(Arc::new(BuiltInManagedValue::IsLastStep)),
        "remaining_steps" => Some(Arc::new(BuiltInManagedValue::RemainingSteps)),
        _ => None,
    }
}

pub fn builtin_managed_values() -> ManagedValueRegistry {
    let mut values = ManagedValueRegistry::new();
    values.insert(
        BuiltInManagedValue::IsLastStep.key().to_owned(),
        Arc::new(BuiltInManagedValue::IsLastStep),
    );
    values.insert(
        BuiltInManagedValue::RemainingSteps.key().to_owned(),
        Arc::new(BuiltInManagedValue::RemainingSteps),
    );
    values
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{BuiltInManagedValue, FnManagedValue, ManagedValue, ManagedValueContext};

    #[test]
    fn builtin_is_last_step_matches_python_semantics() {
        let value = BuiltInManagedValue::IsLastStep.get(ManagedValueContext::new(2, 3));
        assert_eq!(value, json!(true));

        let value = BuiltInManagedValue::IsLastStep.get(ManagedValueContext::new(2, 5));
        assert_eq!(value, json!(false));
    }

    #[test]
    fn builtin_remaining_steps_is_non_negative() {
        let value = BuiltInManagedValue::RemainingSteps.get(ManagedValueContext::new(2, 5));
        assert_eq!(value, json!(3));

        let value = BuiltInManagedValue::RemainingSteps.get(ManagedValueContext::new(5, 2));
        assert_eq!(value, json!(0));
    }

    #[test]
    fn custom_managed_value_resolver_runs() {
        let managed = FnManagedValue::new(|ctx| json!({"step": ctx.step, "stop": ctx.stop}));
        let value = managed.get(ManagedValueContext::new(1, 9));
        assert_eq!(value, json!({"step": 1, "stop": 9}));
    }
}
