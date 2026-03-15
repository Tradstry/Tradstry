use crate::langgraph_rs::core::managed::{BuiltInManagedValue, ManagedValueRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedValueKind {
    IsLastStep,
    RemainingSteps,
}

impl ManagedValueKind {
    pub fn to_managed_value(self) -> ManagedValueRef {
        match self {
            Self::IsLastStep => std::sync::Arc::new(BuiltInManagedValue::IsLastStep),
            Self::RemainingSteps => std::sync::Arc::new(BuiltInManagedValue::RemainingSteps),
        }
    }
}
