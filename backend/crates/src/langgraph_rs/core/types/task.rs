use serde::{Deserialize, Serialize};

use super::{NodeName, TaskId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum TaskPathPart {
    Name(String),
    Index(u64),
    Nested(Vec<TaskPathPart>),
}

pub type TaskPath = Vec<TaskPathPart>;

pub trait TaskPathStr {
    fn to_path_string(&self) -> String;
}

impl TaskPathStr for TaskPathPart {
    fn to_path_string(&self) -> String {
        match self {
            Self::Name(name) => name.clone(),
            Self::Index(index) => format!("{index:010}"),
            Self::Nested(parts) => {
                let inner = parts
                    .iter()
                    .map(TaskPathStr::to_path_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("~{inner}")
            }
        }
    }
}

impl TaskPathStr for TaskPath {
    fn to_path_string(&self) -> String {
        self.iter()
            .map(TaskPathStr::to_path_string)
            .collect::<Vec<_>>()
            .join("::")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDescriptor {
    pub id: TaskId,
    pub name: NodeName,
    #[serde(default)]
    pub path: TaskPath,
}

impl TaskDescriptor {
    pub fn new(id: impl Into<TaskId>, name: impl Into<NodeName>, path: TaskPath) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            path,
        }
    }
}
