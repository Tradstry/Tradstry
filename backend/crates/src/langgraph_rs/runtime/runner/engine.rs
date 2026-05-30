use std::{
    collections::{BTreeMap, VecDeque},
    hash::{DefaultHasher, Hash, Hasher},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use serde_json::Value;

use crate::langgraph_rs::{
    cache::base::{
        Cache, CacheKey, CacheSetOptions, cache_value_to_node_result, node_result_to_cache_value,
    },
    core::{
        constants::TASKS,
        types::{
            ChannelWrite, CommandGraph, ExecutionContext, NodeExecutionError,
            NodeExecutionErrorKind, NodeExecutionResult, RuntimeCapabilities, RuntimeReadSelection,
            SendPacket, TaskDescriptor, TaskPathPart,
        },
    },
    runtime::{
        io::map_command_to_writes,
        r#loop::LoopNodeRunner,
        streaming::{RuntimeEvent, RuntimeStream},
    },
};

use super::{
    RunnerConfig, RunnerError, TaskExecutionRequest, TaskExecutionResult,
    retry::retry_sleep_duration,
};

#[derive(Clone)]
pub struct TaskRunner {
    node_runner: Arc<dyn LoopNodeRunner>,
    config: RunnerConfig,
    stream: Option<Arc<dyn RuntimeStream>>,
    cache: Option<Arc<dyn Cache>>,
}

struct LocalRuntimeCapabilities {
    node_runner: Arc<dyn LoopNodeRunner>,
    task: Arc<TaskDescriptor>,
    step: u64,
    recursion_limit: u64,
    base_values: BTreeMap<String, Value>,
    pending_writes: Mutex<Vec<ChannelWrite>>,
    call_counter: AtomicU64,
    self_ref: Mutex<std::sync::Weak<LocalRuntimeCapabilities>>,
}

impl LocalRuntimeCapabilities {
    fn new(
        node_runner: Arc<dyn LoopNodeRunner>,
        task: Arc<TaskDescriptor>,
        request: &TaskExecutionRequest,
    ) -> Arc<Self> {
        let base_values = resolve_runtime_state_values(request);
        let caps = Arc::new(Self {
            node_runner,
            task,
            step: request.step,
            recursion_limit: request.recursion_limit,
            base_values,
            pending_writes: Mutex::new(Vec::new()),
            call_counter: AtomicU64::new(0),
            self_ref: Mutex::new(std::sync::Weak::new()),
        });
        if let Ok(mut slot) = caps.self_ref.lock() {
            *slot = Arc::downgrade(&caps);
        }
        caps
    }

    fn take_pending_writes(&self) -> Result<Vec<ChannelWrite>, NodeExecutionError> {
        let mut guard = self
            .pending_writes
            .lock()
            .map_err(|_| NodeExecutionError::fatal("runtime capability state lock was poisoned"))?;
        Ok(std::mem::take(&mut *guard))
    }

    fn stage_result_writes(
        &self,
        mut output: NodeExecutionResult,
    ) -> Result<Value, NodeExecutionError> {
        if let Some(command) = output.command.take() {
            if command.graph == CommandGraph::Parent {
                return Err(NodeExecutionError::fatal(
                    "runtime call cannot return parent command",
                ));
            }
            let mapped = map_command_to_writes(&command)
                .map_err(|_| NodeExecutionError::fatal("failed to map runtime call command"))?;
            output.writes.extend(mapped);
        }

        let mut writes = output.writes;
        writes.extend(
            output
                .sends
                .into_iter()
                .map(send_to_tasks_write)
                .collect::<Vec<_>>(),
        );
        self.pregel_send(writes)?;
        Ok(output.return_value.unwrap_or(Value::Null))
    }
}

#[async_trait::async_trait]
impl RuntimeCapabilities for LocalRuntimeCapabilities {
    fn pregel_read(
        &self,
        select: RuntimeReadSelection,
        fresh: bool,
    ) -> Result<Value, NodeExecutionError> {
        let mut values = self.base_values.clone();
        if fresh {
            let pending = self.pending_writes.lock().map_err(|_| {
                NodeExecutionError::fatal("runtime capability state lock was poisoned")
            })?;
            for write in pending.iter() {
                values.insert(write.channel.clone(), write.value.clone());
            }
        }

        Ok(match select {
            RuntimeReadSelection::Single(channel_name) => {
                values.get(&channel_name).cloned().unwrap_or(Value::Null)
            }
            RuntimeReadSelection::Many(channel_names) => {
                let mut object = serde_json::Map::new();
                for channel_name in channel_names {
                    if let Some(value) = values.get(&channel_name) {
                        object.insert(channel_name, value.clone());
                    }
                }
                Value::Object(object)
            }
        })
    }

    fn pregel_send(&self, writes: Vec<ChannelWrite>) -> Result<(), NodeExecutionError> {
        if writes.is_empty() {
            return Ok(());
        }
        let mut pending = self
            .pending_writes
            .lock()
            .map_err(|_| NodeExecutionError::fatal("runtime capability state lock was poisoned"))?;
        pending.extend(writes);
        Ok(())
    }

    async fn pregel_call(
        &self,
        node_name: &str,
        input: Value,
    ) -> Result<Value, NodeExecutionError> {
        let call_idx = self.call_counter.fetch_add(1, Ordering::SeqCst);
        let descriptor = TaskDescriptor::new(
            format!("{}:call:{call_idx}:{node_name}", self.task.id),
            node_name.to_owned(),
            vec![
                TaskPathPart::Name("call".to_owned()),
                TaskPathPart::Nested(self.task.path.clone()),
                TaskPathPart::Name(node_name.to_owned()),
                TaskPathPart::Index(call_idx),
            ],
        );

        let runtime: Option<Arc<dyn RuntimeCapabilities>> = self
            .self_ref
            .lock()
            .ok()
            .and_then(|weak| weak.upgrade())
            .map(|arc| arc as Arc<dyn RuntimeCapabilities>);

        let ctx = ExecutionContext {
            task: Arc::new(descriptor),
            step: self.step,
            recursion_limit: self.recursion_limit,
            attempt: 1,
            resuming: false,
            runtime,
        };
        let output = self.node_runner.execute(node_name, input, ctx).await?;
        self.stage_result_writes(output)
    }
}

impl TaskRunner {
    pub fn new(node_runner: Arc<dyn LoopNodeRunner>, config: RunnerConfig) -> Self {
        Self {
            node_runner,
            config,
            stream: None,
            cache: None,
        }
    }

    pub fn with_stream(mut self, stream: Option<Arc<dyn RuntimeStream>>) -> Self {
        self.stream = stream;
        self
    }

    pub fn with_cache(mut self, cache: Option<Arc<dyn Cache>>) -> Self {
        self.cache = cache;
        self
    }

    pub async fn execute(
        &self,
        request: TaskExecutionRequest,
    ) -> Result<TaskExecutionResult, RunnerError> {
        self.execute_one(request).await
    }

    pub async fn execute_one(
        &self,
        request: TaskExecutionRequest,
    ) -> Result<TaskExecutionResult, RunnerError> {
        let stream = self.stream.as_deref();
        let cache = self.cache.as_deref();
        let cache_enabled = request.cache_enabled.unwrap_or(true);
        let cache_key = if cache_enabled {
            cache.map(|_| cache_key_for_request(&self.config, &request))
        } else {
            None
        };
        if let (Some(cache), Some(cache_key)) = (cache, cache_key.as_ref()) {
            match cache.aget(cache_key).await {
                Ok(Some(item)) => {
                    if let Ok(output) = cache_value_to_node_result(&item.value) {
                        emit(
                            stream,
                            RuntimeEvent::TaskCacheHit {
                                step: request.step,
                                task_id: request.task.id.clone(),
                                node: request.task.name.clone(),
                            },
                        );
                        emit(
                            stream,
                            RuntimeEvent::TaskSucceeded {
                                step: request.step,
                                task_id: request.task.id.clone(),
                                node: request.task.name.clone(),
                                attempts: 0,
                                writes: output.writes.len(),
                                sends: output.sends.len(),
                                has_return_value: output.return_value.is_some(),
                            },
                        );
                        return Ok(TaskExecutionResult {
                            task: request.task,
                            output,
                            attempts: 0,
                            cached: true,
                        });
                    }
                    emit(
                        stream,
                        RuntimeEvent::TaskCacheMiss {
                            step: request.step,
                            task_id: request.task.id.clone(),
                            node: request.task.name.clone(),
                        },
                    );
                }
                Ok(None) | Err(_) => {
                    emit(
                        stream,
                        RuntimeEvent::TaskCacheMiss {
                            step: request.step,
                            task_id: request.task.id.clone(),
                            node: request.task.name.clone(),
                        },
                    );
                }
            }
        }

        let retry_policies = self
            .config
            .effective_retry_policies(request.retry_policies.as_deref());
        let mut attempts: u32 = 0;

        let task_arc = Arc::new(request.task.clone());
        loop {
            attempts = attempts.saturating_add(1);
            let runtime_caps = LocalRuntimeCapabilities::new(
                Arc::clone(&self.node_runner),
                Arc::clone(&task_arc),
                &request,
            );
            let ctx = ExecutionContext {
                task: Arc::clone(&task_arc),
                step: request.step,
                recursion_limit: request.recursion_limit,
                attempt: attempts,
                resuming: attempts > 1,
                runtime: Some(Arc::clone(&runtime_caps) as Arc<dyn RuntimeCapabilities>),
            };
            emit(
                stream,
                RuntimeEvent::TaskStarted {
                    step: request.step,
                    task_id: request.task.id.clone(),
                    node: request.task.name.clone(),
                    attempt: attempts,
                },
            );

            match self
                .node_runner
                .execute(&request.task.name, request.input.clone(), ctx)
                .await
            {
                Ok(mut output) => {
                    let runtime_writes = runtime_caps.take_pending_writes().map_err(|err| {
                        RunnerError::Execution {
                            task_id: request.task.id.clone(),
                            node: request.task.name.clone(),
                            attempts,
                            kind: err.kind,
                            message: err.message,
                        }
                    })?;
                    output.writes.extend(runtime_writes);

                    if let Some(command) = output.command.clone() {
                        if command.graph == CommandGraph::Parent {
                            return Err(RunnerError::ParentCommand {
                                task_id: request.task.id,
                                node: request.task.name,
                                command: Box::new(command),
                            });
                        }
                        let mapped = map_command_to_writes(&command).map_err(|_| {
                            RunnerError::Execution {
                                task_id: request.task.id.clone(),
                                node: request.task.name.clone(),
                                attempts,
                                kind: NodeExecutionErrorKind::Fatal,
                                message: "failed to map node command".to_owned(),
                            }
                        })?;
                        output.writes.extend(mapped);
                        output.command = None;
                    }

                    if let (Some(cache), Some(cache_key)) = (cache, cache_key.as_ref()) {
                        let mut metadata = BTreeMap::new();
                        metadata.insert(
                            "task_id".to_owned(),
                            serde_json::json!(request.task.id.clone()),
                        );
                        metadata.insert(
                            "node".to_owned(),
                            serde_json::json!(request.task.name.clone()),
                        );
                        metadata.insert("step".to_owned(), serde_json::json!(request.step));
                        let mut options = CacheSetOptions::new().with_metadata(metadata);
                        if let Some(ttl_millis) = self.config.cache_ttl_millis {
                            options = options.with_ttl_millis(ttl_millis);
                        }

                        let cached_value = node_result_to_cache_value(&output);
                        if cache.aset(cache_key, cached_value, options).await.is_ok() {
                            emit(
                                stream,
                                RuntimeEvent::TaskCacheStored {
                                    step: request.step,
                                    task_id: request.task.id.clone(),
                                    node: request.task.name.clone(),
                                    ttl_millis: self.config.cache_ttl_millis,
                                },
                            );
                        }
                    }
                    emit(
                        stream,
                        RuntimeEvent::TaskSucceeded {
                            step: request.step,
                            task_id: request.task.id.clone(),
                            node: request.task.name.clone(),
                            attempts,
                            writes: output.writes.len(),
                            sends: output.sends.len(),
                            has_return_value: output.return_value.is_some(),
                        },
                    );
                    return Ok(TaskExecutionResult {
                        task: request.task,
                        output,
                        attempts,
                        cached: false,
                    });
                }
                Err(err) => {
                    if let Some(policy) = first_retry_policy(&retry_policies, &err)
                        && attempts < policy.max_attempts
                    {
                        emit(
                            stream,
                            RuntimeEvent::TaskRetrying {
                                step: request.step,
                                task_id: request.task.id.clone(),
                                node: request.task.name.clone(),
                                attempt: attempts,
                                message: err.message.clone(),
                            },
                        );
                        tokio::time::sleep(retry_sleep_duration(policy, attempts)).await;
                        continue;
                    }
                    emit(
                        stream,
                        RuntimeEvent::TaskFailed {
                            step: request.step,
                            task_id: request.task.id.clone(),
                            node: request.task.name.clone(),
                            attempts,
                            kind: error_kind(err.kind).to_owned(),
                            message: err.message.clone(),
                        },
                    );
                    return Err(RunnerError::Execution {
                        task_id: request.task.id,
                        node: request.task.name,
                        attempts,
                        kind: err.kind,
                        message: err.message,
                    });
                }
            }
        }
    }

    pub async fn execute_many_progressive<F>(
        self: Arc<Self>,
        initial_requests: Vec<TaskExecutionRequest>,
        max_concurrency: usize,
        mut on_complete: F,
    ) -> Result<(), RunnerError>
    where
        F: FnMut(TaskExecutionResult) -> Result<Vec<TaskExecutionRequest>, RunnerError>,
    {
        let max_concurrency = max_concurrency.max(1);
        let mut pending = VecDeque::from(initial_requests);
        let mut join_set: tokio::task::JoinSet<Result<TaskExecutionResult, RunnerError>> =
            tokio::task::JoinSet::new();
        let mut terminal_error = None::<RunnerError>;

        loop {
            while terminal_error.is_none()
                && join_set.len() < max_concurrency
                && !pending.is_empty()
            {
                let request = pending.pop_front().expect("request must exist");
                let runner = Arc::clone(&self);
                join_set.spawn(async move { runner.execute_one(request).await });
            }

            if join_set.is_empty() {
                break;
            }

            match join_set.join_next().await {
                Some(Ok(Ok(completed))) => {
                    if terminal_error.is_none() {
                        match on_complete(completed) {
                            Ok(reqs) => pending.extend(reqs),
                            Err(e) => {
                                terminal_error = Some(e);
                                join_set.abort_all();
                            }
                        }
                    }
                }
                Some(Ok(Err(e))) => {
                    if terminal_error.is_none() {
                        terminal_error = Some(e);
                        join_set.abort_all();
                    }
                }
                Some(Err(join_err)) => {
                    if terminal_error.is_none() {
                        terminal_error = Some(RunnerError::from_join_error(join_err));
                        join_set.abort_all();
                    }
                }
                None => break,
            }
        }

        match terminal_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

fn first_retry_policy<'a>(
    policies: &'a [crate::langgraph_rs::runtime::runner::RetryPolicy],
    err: &NodeExecutionError,
) -> Option<&'a crate::langgraph_rs::runtime::runner::RetryPolicy> {
    policies
        .iter()
        .find(|policy| policy.max_attempts > 0 && policy.retry_on.matches(err))
}

fn emit(stream: Option<&dyn RuntimeStream>, event: RuntimeEvent) {
    if let Some(stream) = stream {
        stream.emit(event.into_stream_event());
    }
}

fn cache_key_for_request(config: &RunnerConfig, request: &TaskExecutionRequest) -> CacheKey {
    if let Some(builder) = &config.cache_key_builder {
        return CacheKey::new(config.cache_namespace.clone(), builder(request));
    }

    let canonical_input = canonical_json_string(&request.input);
    let mut hasher = DefaultHasher::new();
    canonical_input.hash(&mut hasher);
    let digest = format!("{:016x}", hasher.finish());
    let cache_key = format!("node={};input_hash={digest}", request.task.name);
    CacheKey::new(config.cache_namespace.clone(), cache_key)
}

fn resolve_runtime_state_values(request: &TaskExecutionRequest) -> BTreeMap<String, Value> {
    if let Some(runtime_state) = &request.runtime_state {
        return runtime_state.values.clone();
    }

    match &request.input {
        Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => BTreeMap::new(),
    }
}

fn send_to_tasks_write(send: SendPacket) -> ChannelWrite {
    let value = serde_json::to_value(send).unwrap_or(Value::Null);
    ChannelWrite::new(TASKS, value)
}

fn canonical_json_string(value: &serde_json::Value) -> String {
    let canonical = canonicalize_json(value);
    serde_json::to_string(&canonical).unwrap_or_else(|_| "null".to_owned())
}

fn canonicalize_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted = std::collections::BTreeMap::<String, serde_json::Value>::new();
            for (key, val) in map {
                sorted.insert(key.clone(), canonicalize_json(val));
            }
            let mut ordered = serde_json::Map::new();
            for (key, val) in sorted {
                ordered.insert(key, val);
            }
            serde_json::Value::Object(ordered)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonicalize_json).collect())
        }
        _ => value.clone(),
    }
}

fn error_kind(kind: NodeExecutionErrorKind) -> &'static str {
    match kind {
        NodeExecutionErrorKind::Retryable => "retryable",
        NodeExecutionErrorKind::Fatal => "fatal",
        NodeExecutionErrorKind::Interrupted => "interrupted",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    };

    use serde_json::json;

    use crate::langgraph_rs::{
        cache::memory::InMemoryCache,
        core::types::{
            Command, ExecutionContext, NodeExecutionError, NodeExecutionErrorKind,
            NodeExecutionResult, TaskDescriptor, TaskPathPart,
        },
        runtime::{
            r#loop::LoopNodeRunner,
            runner::{
                RetryOn, RetryPolicy, RunnerConfig, RunnerError, TaskExecutionRequest, TaskRunner,
            },
            streaming::StreamCollector,
        },
    };

    struct FlakyRunner {
        attempts: Arc<AtomicU32>,
    }

    #[async_trait::async_trait]
    impl LoopNodeRunner for FlakyRunner {
        async fn execute(
            &self,
            _node_name: &str,
            _input: serde_json::Value,
            _ctx: ExecutionContext,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            let current = self.attempts.fetch_add(1, Ordering::SeqCst);
            if current == 0 {
                Err(NodeExecutionError::retryable("try again").with_error_class("network"))
            } else {
                Ok(NodeExecutionResult::default().with_return_value(json!("ok")))
            }
        }
    }

    struct AlwaysRetryableRunner;

    #[async_trait::async_trait]
    impl LoopNodeRunner for AlwaysRetryableRunner {
        async fn execute(
            &self,
            _node_name: &str,
            _input: serde_json::Value,
            _ctx: ExecutionContext,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            Err(NodeExecutionError::retryable("still failing").with_error_class("network"))
        }
    }

    struct CountingSuccessRunner {
        calls: Arc<AtomicU32>,
    }

    #[async_trait::async_trait]
    impl LoopNodeRunner for CountingSuccessRunner {
        async fn execute(
            &self,
            _node_name: &str,
            _input: serde_json::Value,
            _ctx: ExecutionContext,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(NodeExecutionResult::default().with_return_value(json!("ok")))
        }
    }

    struct ErrorCodeRunner {
        attempts: Arc<AtomicU32>,
    }

    #[async_trait::async_trait]
    impl LoopNodeRunner for ErrorCodeRunner {
        async fn execute(
            &self,
            _node_name: &str,
            _input: serde_json::Value,
            _ctx: ExecutionContext,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            let current = self.attempts.fetch_add(1, Ordering::SeqCst);
            if current == 0 {
                return Err(NodeExecutionError::retryable("server error")
                    .with_error_class("http")
                    .with_error_code("500"));
            }
            Ok(NodeExecutionResult::default().with_return_value(json!("ok")))
        }
    }

    #[tokio::test]
    async fn retries_retryable_errors_until_success() {
        let attempts = Arc::new(AtomicU32::new(0));
        let runner = FlakyRunner {
            attempts: attempts.clone(),
        };
        let policy = RetryPolicy::new()
            .with_retry_on(RetryOn::ErrorClasses(vec!["network".to_owned()]))
            .with_max_attempts(3)
            .with_jitter(false);
        let task_runner = TaskRunner::new(
            Arc::new(runner) as Arc<dyn LoopNodeRunner>,
            RunnerConfig::new().with_retry_policies(vec![policy]),
        );

        let result = task_runner
            .execute(TaskExecutionRequest {
                task: TaskDescriptor::new(
                    "t1",
                    "node",
                    vec![TaskPathPart::Name("pull".to_owned())],
                ),
                input: json!({"x": 1}),
                step: 0,
                recursion_limit: 10,
                retry_policies: None,
                cache_enabled: None,
                runtime_state: None,
            })
            .await
            .unwrap();

        assert_eq!(result.attempts, 2);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn retries_by_error_code_matcher() {
        let attempts = Arc::new(AtomicU32::new(0));
        let runner = ErrorCodeRunner {
            attempts: attempts.clone(),
        };
        let policy = RetryPolicy::new()
            .with_retry_on(RetryOn::ErrorCodes(vec!["500".to_owned()]))
            .with_max_attempts(2)
            .with_jitter(false);
        let task_runner = TaskRunner::new(
            Arc::new(runner) as Arc<dyn LoopNodeRunner>,
            RunnerConfig::new().with_retry_policies(vec![policy]),
        );

        let result = task_runner
            .execute(TaskExecutionRequest {
                task: TaskDescriptor::new(
                    "t-code",
                    "node",
                    vec![TaskPathPart::Name("pull".to_owned())],
                ),
                input: json!({}),
                step: 0,
                recursion_limit: 10,
                retry_policies: None,
                cache_enabled: None,
                runtime_state: None,
            })
            .await
            .unwrap();

        assert_eq!(result.attempts, 2);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn returns_error_after_retry_budget_is_exhausted() {
        let runner = AlwaysRetryableRunner;
        let task_runner = TaskRunner::new(
            Arc::new(runner) as Arc<dyn LoopNodeRunner>,
            RunnerConfig::new().with_retry_policies(vec![
                RetryPolicy::new()
                    .with_retry_on(RetryOn::retryable_kind())
                    .with_max_attempts(2)
                    .with_jitter(false),
            ]),
        );

        let error = task_runner
            .execute(TaskExecutionRequest {
                task: TaskDescriptor::new(
                    "t2",
                    "node",
                    vec![TaskPathPart::Name("pull".to_owned())],
                ),
                input: json!({}),
                step: 0,
                recursion_limit: 10,
                retry_policies: None,
                cache_enabled: None,
                runtime_state: None,
            })
            .await
            .unwrap_err();

        match error {
            RunnerError::Execution { attempts, kind, .. } => {
                assert_eq!(attempts, 2);
                assert_eq!(kind, NodeExecutionErrorKind::Retryable);
            }
            _ => panic!("unexpected error kind"),
        }
    }

    #[tokio::test]
    async fn emits_structured_task_lifecycle_events() {
        let attempts = Arc::new(AtomicU32::new(0));
        let runner = FlakyRunner {
            attempts: attempts.clone(),
        };
        let collector = Arc::new(StreamCollector::new());
        let task_runner = TaskRunner::new(
            Arc::new(runner) as Arc<dyn LoopNodeRunner>,
            RunnerConfig::new().with_retry_policies(vec![
                RetryPolicy::new()
                    .with_retry_on(RetryOn::ErrorClasses(vec!["network".to_owned()]))
                    .with_max_attempts(3)
                    .with_jitter(false),
            ]),
        )
        .with_stream(Some(Arc::clone(&collector)
            as Arc<dyn crate::langgraph_rs::runtime::streaming::RuntimeStream>));

        let _result = task_runner
            .execute(TaskExecutionRequest {
                task: TaskDescriptor::new(
                    "t3",
                    "node",
                    vec![TaskPathPart::Name("pull".to_owned())],
                ),
                input: json!({}),
                step: 2,
                recursion_limit: 10,
                retry_policies: None,
                cache_enabled: None,
                runtime_state: None,
            })
            .await
            .unwrap();

        let events = collector.events();
        assert!(events.iter().any(
            |event| event.payload.get("event").and_then(|v| v.as_str()) == Some("task_started")
        ));
        assert!(
            events
                .iter()
                .any(|event| event.payload.get("event").and_then(|v| v.as_str())
                    == Some("task_retrying"))
        );
        assert!(
            events
                .iter()
                .any(|event| event.payload.get("event").and_then(|v| v.as_str())
                    == Some("task_succeeded"))
        );
    }

    #[tokio::test]
    async fn uses_cache_for_task_level_memoization() {
        let calls = Arc::new(AtomicU32::new(0));
        let runner = CountingSuccessRunner {
            calls: calls.clone(),
        };
        let cache = Arc::new(InMemoryCache::new());
        let collector = Arc::new(StreamCollector::new());
        let config = RunnerConfig::new()
            .with_retry_limit(0)
            .with_cache_ttl_millis(60_000);
        let task_runner = TaskRunner::new(Arc::new(runner) as Arc<dyn LoopNodeRunner>, config)
            .with_cache(Some(
                Arc::clone(&cache) as Arc<dyn crate::langgraph_rs::cache::base::Cache>
            ))
            .with_stream(Some(Arc::clone(&collector)
                as Arc<dyn crate::langgraph_rs::runtime::streaming::RuntimeStream>));
        let request = TaskExecutionRequest {
            task: TaskDescriptor::new(
                "t-cache",
                "node",
                vec![TaskPathPart::Name("pull".to_owned())],
            ),
            input: json!({"x": 1}),
            step: 7,
            recursion_limit: 10,
            retry_policies: None,
            cache_enabled: None,
            runtime_state: None,
        };

        let first = task_runner.execute(request.clone()).await.unwrap();
        let second = task_runner.execute(request).await.unwrap();

        assert_eq!(first.attempts, 1);
        assert_eq!(second.attempts, 0);
        assert!(second.cached);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let events = collector.events();
        let has = |name: &str| {
            events.iter().any(|event| {
                event.payload.get("event").and_then(|value| value.as_str()) == Some(name)
            })
        };

        assert!(has("task_cache_miss"));
        assert!(has("task_cache_stored"));
        assert!(has("task_cache_hit"));
    }

    #[tokio::test]
    async fn cache_key_is_stable_across_task_id_and_step() {
        let calls = Arc::new(AtomicU32::new(0));
        let runner = CountingSuccessRunner {
            calls: calls.clone(),
        };
        let cache = Arc::new(InMemoryCache::new());
        let task_runner = TaskRunner::new(
            Arc::new(runner) as Arc<dyn LoopNodeRunner>,
            RunnerConfig::new()
                .with_retry_limit(0)
                .with_cache_ttl_millis(60_000),
        )
        .with_cache(Some(
            Arc::clone(&cache) as Arc<dyn crate::langgraph_rs::cache::base::Cache>
        ));

        let first = task_runner
            .execute(TaskExecutionRequest {
                task: TaskDescriptor::new(
                    "t-cache-a",
                    "node",
                    vec![TaskPathPart::Name("pull".to_owned())],
                ),
                input: json!({"x": 1}),
                step: 1,
                recursion_limit: 10,
                retry_policies: None,
                cache_enabled: None,
                runtime_state: None,
            })
            .await
            .unwrap();
        let second = task_runner
            .execute(TaskExecutionRequest {
                task: TaskDescriptor::new(
                    "t-cache-b",
                    "node",
                    vec![TaskPathPart::Name("pull".to_owned())],
                ),
                input: json!({"x": 1}),
                step: 999,
                recursion_limit: 10,
                retry_policies: None,
                cache_enabled: None,
                runtime_state: None,
            })
            .await
            .unwrap();

        assert_eq!(first.attempts, 1);
        assert_eq!(second.attempts, 0);
        assert!(second.cached);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    struct ParentCommandRunner;

    #[async_trait::async_trait]
    impl LoopNodeRunner for ParentCommandRunner {
        async fn execute(
            &self,
            _node_name: &str,
            _input: serde_json::Value,
            _ctx: ExecutionContext,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            Ok(NodeExecutionResult::default().with_command(Command::new().to_parent()))
        }
    }

    #[tokio::test]
    async fn bubbles_parent_command_without_retry() {
        let runner = ParentCommandRunner;
        let task_runner = TaskRunner::new(
            Arc::new(runner) as Arc<dyn LoopNodeRunner>,
            RunnerConfig::new().with_retry_limit(2),
        );

        let err = task_runner
            .execute(TaskExecutionRequest {
                task: TaskDescriptor::new(
                    "t-parent",
                    "node",
                    vec![TaskPathPart::Name("pull".to_owned())],
                ),
                input: json!({}),
                step: 0,
                recursion_limit: 10,
                retry_policies: None,
                cache_enabled: None,
                runtime_state: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, RunnerError::ParentCommand { .. }));
    }

    #[tokio::test]
    async fn execute_many_progressive_runs_to_completion() {
        let calls = Arc::new(AtomicU32::new(0));
        let runner = CountingSuccessRunner {
            calls: calls.clone(),
        };
        let task_runner = TaskRunner::new(
            Arc::new(runner) as Arc<dyn LoopNodeRunner>,
            RunnerConfig::new().with_max_concurrency(2),
        );
        let requests = vec![
            TaskExecutionRequest {
                task: TaskDescriptor::new(
                    "t1",
                    "node",
                    vec![TaskPathPart::Name("pull".to_owned())],
                ),
                input: json!({"x": 1}),
                step: 0,
                recursion_limit: 10,
                retry_policies: None,
                cache_enabled: Some(false),
                runtime_state: None,
            },
            TaskExecutionRequest {
                task: TaskDescriptor::new(
                    "t2",
                    "node",
                    vec![TaskPathPart::Name("pull".to_owned())],
                ),
                input: json!({"x": 2}),
                step: 0,
                recursion_limit: 10,
                retry_policies: None,
                cache_enabled: Some(false),
                runtime_state: None,
            },
        ];

        let mut completed = 0usize;
        Arc::new(task_runner)
            .execute_many_progressive(requests, 2, |_result| {
                completed = completed.saturating_add(1);
                Ok(Vec::new())
            })
            .await
            .unwrap();

        assert_eq!(completed, 2);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
