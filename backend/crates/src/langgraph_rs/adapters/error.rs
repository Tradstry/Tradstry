use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdapterError {
    #[error("adapter node name cannot be empty")]
    InvalidNodeName,
    #[error("adapter node '{0}' is already registered")]
    DuplicateNode(String),
    #[error("adapter node '{0}' is not registered")]
    MissingNode(String),
}

impl AdapterError {
    pub fn duplicate_node(node_name: impl Into<String>) -> Self {
        Self::DuplicateNode(node_name.into())
    }

    pub fn missing_node(node_name: impl Into<String>) -> Self {
        Self::MissingNode(node_name.into())
    }
}
