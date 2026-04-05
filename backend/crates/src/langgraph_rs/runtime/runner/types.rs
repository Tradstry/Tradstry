use std::sync::Arc;
use std::{collections::BTreeMap, fmt};

use serde_json::Value;

use crate::langgraph_rs::{
    cache::base::CacheNamespace,
    core::types::{
        NodeExecutionError, NodeExecutionErrorKind, NodeExecutionResult, TaskDescriptor,
    },
};

pub type RetryPredicate = Arc<dyn Fn(&NodeExecutionError) -> bool + Send + Sync>;
pub type CacheKeyBuilder = Arc<dyn Fn(&TaskExecutionRequest) -> String + Send + Sync>;

#[derive(Clone)]
pub enum RetryOn {
    ErrorClasses(Vec<String>),
    ErrorCodes(Vec<String>),
    ErrorKinds(Vec<NodeExecutionErrorKind>),
    Predicate(RetryPredicate),
}

impl fmt::Debug for RetryOn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ErrorClasses(values) => f.debug_tuple("ErrorClasses").field(values).finish(),
            Self::ErrorCodes(values) => f.debug_tuple("ErrorCodes").field(values).finish(),
            Self::ErrorKinds(values) => f.debug_tuple("ErrorKinds").field(values).finish(),
            Self::Predicate(_) => f.write_str("Predicate(<fn>)"),
        }
    }
}

impl RetryOn {
    pub fn matches(&self, err: &NodeExecutionError) -> bool {
        match self {
            Self::ErrorClasses(classes) => classes.iter().any(|class| class == &err.error_class),
            Self::ErrorCodes(codes) => err
                .error_code
                .as_ref()
                .is_some_and(|code| codes.iter().any(|candidate| candidate == code)),
            Self::ErrorKinds(kinds) => kinds.iter().any(|kind| kind == &err.kind),
            Self::Predicate(predicate) => predicate(err),
        }
    }

    pub fn retryable_kind() -> Self {
        Self::ErrorKinds(vec![NodeExecutionErrorKind::Retryable])
    }
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub initial_interval_secs: f64,
    pub backoff_factor: f64,
    pub max_interval_secs: f64,
    pub max_attempts: u32,
    pub jitter: bool,
    pub retry_on: RetryOn,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            initial_interval_secs: 0.5,
            backoff_factor: 2.0,
            max_interval_secs: 128.0,
            max_attempts: 3,
            jitter: true,
            retry_on: RetryOn::retryable_kind(),
        }
    }
}

impl RetryPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_retry_on(mut self, retry_on: RetryOn) -> Self {
        self.retry_on = retry_on;
        self
    }

    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
    }

    pub fn with_initial_interval_secs(mut self, initial_interval_secs: f64) -> Self {
        self.initial_interval_secs = initial_interval_secs.max(0.0);
        self
    }

    pub fn with_backoff_factor(mut self, backoff_factor: f64) -> Self {
        self.backoff_factor = backoff_factor.max(1.0);
        self
    }

    pub fn with_max_interval_secs(mut self, max_interval_secs: f64) -> Self {
        self.max_interval_secs = max_interval_secs.max(0.0);
        self
    }

    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    pub fn from_retry_limit(retry_limit: u32) -> Self {
        Self::new()
            .with_retry_on(RetryOn::retryable_kind())
            .with_max_attempts(retry_limit.saturating_add(1))
            .with_jitter(false)
    }
}

#[derive(Clone)]
pub struct RunnerConfig {
    pub retry_policies: Vec<RetryPolicy>,
    pub retry_limit_compat: u32,
    pub max_concurrency: usize,
    pub cache_namespace: CacheNamespace,
    pub cache_ttl_millis: Option<u64>,
    pub cache_key_builder: Option<CacheKeyBuilder>,
}

impl fmt::Debug for RunnerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunnerConfig")
            .field("retry_policies", &self.retry_policies)
            .field("retry_limit_compat", &self.retry_limit_compat)
            .field("max_concurrency", &self.max_concurrency)
            .field("cache_namespace", &self.cache_namespace)
            .field("cache_ttl_millis", &self.cache_ttl_millis)
            .field(
                "cache_key_builder",
                &self.cache_key_builder.as_ref().map(|_| "<fn>"),
            )
            .finish()
    }
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            retry_policies: Vec::new(),
            retry_limit_compat: 0,
            max_concurrency: std::thread::available_parallelism()
                .map(|value| value.get())
                .unwrap_or(1)
                .max(1),
            cache_namespace: vec!["runtime".to_owned(), "runner".to_owned(), "task".to_owned()],
            cache_ttl_millis: None,
            cache_key_builder: None,
        }
    }
}

impl RunnerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_retry_policies(mut self, retry_policies: Vec<RetryPolicy>) -> Self {
        self.retry_policies = retry_policies;
        self
    }

    pub fn with_retry_limit(mut self, retry_limit: u32) -> Self {
        self.retry_limit_compat = retry_limit;
        self
    }

    pub fn with_cache_namespace(mut self, cache_namespace: CacheNamespace) -> Self {
        self.cache_namespace = cache_namespace;
        self
    }

    pub fn with_cache_ttl_millis(mut self, cache_ttl_millis: u64) -> Self {
        self.cache_ttl_millis = Some(cache_ttl_millis);
        self
    }

    pub fn with_max_concurrency(mut self, max_concurrency: usize) -> Self {
        self.max_concurrency = max_concurrency.max(1);
        self
    }

    pub fn with_cache_key_builder(mut self, cache_key_builder: CacheKeyBuilder) -> Self {
        self.cache_key_builder = Some(cache_key_builder);
        self
    }

    pub fn effective_retry_policies(
        &self,
        override_policies: Option<&[RetryPolicy]>,
    ) -> Vec<RetryPolicy> {
        if let Some(policies) = override_policies
            && !policies.is_empty()
        {
            return policies.to_vec();
        }
        if !self.retry_policies.is_empty() {
            return self.retry_policies.clone();
        }
        vec![RetryPolicy::from_retry_limit(self.retry_limit_compat)]
    }
}

#[derive(Debug, Clone)]
pub struct TaskRuntimeState {
    pub values: BTreeMap<String, Value>,
}

impl TaskRuntimeState {
    pub fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    pub fn from_values(values: BTreeMap<String, Value>) -> Self {
        Self { values }
    }

    pub fn with_value(mut self, key: impl Into<String>, value: Value) -> Self {
        self.values.insert(key.into(), value);
        self
    }
}

impl Default for TaskRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct TaskExecutionRequest {
    pub task: TaskDescriptor,
    pub input: Value,
    pub step: u64,
    pub recursion_limit: u64,
    pub retry_policies: Option<Vec<RetryPolicy>>,
    pub cache_enabled: Option<bool>,
    pub runtime_state: Option<TaskRuntimeState>,
}

impl TaskExecutionRequest {
    pub fn with_retry_policies(mut self, retry_policies: Vec<RetryPolicy>) -> Self {
        self.retry_policies = Some(retry_policies);
        self
    }

    pub fn with_cache_enabled(mut self, cache_enabled: bool) -> Self {
        self.cache_enabled = Some(cache_enabled);
        self
    }

    pub fn with_runtime_state(mut self, runtime_state: TaskRuntimeState) -> Self {
        self.runtime_state = Some(runtime_state);
        self
    }
}

#[derive(Debug, Clone)]
pub struct TaskExecutionResult {
    pub task: TaskDescriptor,
    pub output: NodeExecutionResult,
    pub attempts: u32,
    pub cached: bool,
}
