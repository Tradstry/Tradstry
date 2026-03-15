use std::collections::BTreeSet;

use crate::langgraph_rs::core::types::TaskDescriptor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterruptSelector {
    None,
    All,
    Nodes(BTreeSet<String>),
}

impl Default for InterruptSelector {
    fn default() -> Self {
        Self::None
    }
}

impl InterruptSelector {
    pub fn none() -> Self {
        Self::None
    }

    pub fn all() -> Self {
        Self::All
    }

    pub fn nodes(nodes: impl IntoIterator<Item = String>) -> Self {
        Self::Nodes(nodes.into_iter().collect())
    }

    pub fn matches_task(&self, task: &TaskDescriptor) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Nodes(nodes) => nodes.contains(&task.name),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::None)
    }
}

#[cfg(test)]
mod tests {
    use crate::langgraph_rs::core::types::{TaskDescriptor, TaskPathPart};

    use super::InterruptSelector;

    #[test]
    fn selector_matches_expected_tasks() {
        let task = TaskDescriptor::new("t1", "node_a", vec![TaskPathPart::Name("pull".to_owned())]);

        assert!(!InterruptSelector::none().matches_task(&task));
        assert!(InterruptSelector::all().matches_task(&task));
        assert!(InterruptSelector::nodes(vec!["node_a".to_owned()]).matches_task(&task));
        assert!(!InterruptSelector::nodes(vec!["node_b".to_owned()]).matches_task(&task));
    }
}
