use std::sync::Arc;

use langgraph::prelude::{
    CheckpointSaver, InMemorySaver, PostgresSaver,
    Checkpoint, CheckpointConfig, CheckpointError, CheckpointMetadata,
    CheckpointTuple, ListCheckpointsQuery, ChannelVersions, PruneStrategy,
};
use langgraph::core::types::{ChannelName, TaskId};
use log::{info, warn};
use serde_json::Value;

/// A wrapper around PostgresSaver that delegates all calls to a dedicated OS thread
/// to avoid the "cannot start runtime from within a runtime" panic. The synchronous
/// `postgres` crate internally uses `block_on`, which panics inside tokio.
struct BlockingPostgresSaver {
    inner: Arc<PostgresSaver>,
}

impl BlockingPostgresSaver {
    fn new(saver: PostgresSaver) -> Self {
        Self {
            inner: Arc::new(saver),
        }
    }

    /// Run a closure on a fresh OS thread (not a tokio thread) to avoid runtime nesting.
    fn block<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&PostgresSaver) -> R + Send + 'static,
        R: Send + 'static,
    {
        let inner = self.inner.clone();
        std::thread::scope(|s| {
            s.spawn(move || f(&inner)).join().expect("blocking checkpoint thread panicked")
        })
    }
}

impl CheckpointSaver for BlockingPostgresSaver {
    fn get(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, CheckpointError> {
        let config = config.clone();
        self.block(move |saver| saver.get(&config))
    }

    fn get_tuple(&self, config: &CheckpointConfig) -> Result<Option<CheckpointTuple>, CheckpointError> {
        let config = config.clone();
        self.block(move |saver| saver.get_tuple(&config))
    }

    fn list(&self, query: &ListCheckpointsQuery) -> Result<Vec<CheckpointTuple>, CheckpointError> {
        let query = query.clone();
        self.block(move |saver| saver.list(&query))
    }

    fn put(
        &self,
        config: &CheckpointConfig,
        checkpoint: Checkpoint,
        metadata: CheckpointMetadata,
        new_versions: ChannelVersions,
    ) -> Result<CheckpointConfig, CheckpointError> {
        let config = config.clone();
        self.block(move |saver| saver.put(&config, checkpoint, metadata, new_versions))
    }

    fn put_writes(
        &self,
        config: &CheckpointConfig,
        writes: &[(ChannelName, Value)],
        task_id: &TaskId,
        task_path: &str,
    ) -> Result<(), CheckpointError> {
        let config = config.clone();
        let writes: Vec<(ChannelName, Value)> = writes.to_vec();
        let task_id = task_id.clone();
        let task_path = task_path.to_string();
        self.block(move |saver| saver.put_writes(&config, &writes, &task_id, &task_path))
    }

    fn delete_thread(&self, thread_id: &str) -> Result<(), CheckpointError> {
        let thread_id = thread_id.to_string();
        self.block(move |saver| saver.delete_thread(&thread_id))
    }

    fn prune(
        &self,
        thread_ids: &[String],
        strategy: PruneStrategy,
    ) -> Result<(), CheckpointError> {
        let thread_ids = thread_ids.to_vec();
        self.block(move |saver| saver.prune(&thread_ids, strategy))
    }
}

/// Initialize checkpoint saver: try Postgres first, fall back to in-memory.
pub async fn init_checkpoint_saver() -> Arc<dyn CheckpointSaver> {
    match std::env::var("POSTGRES_URL") {
        Ok(url) if !url.is_empty() => {
            let result = tokio::task::spawn_blocking(move || {
                PostgresSaver::new(&url)
            })
            .await;

            match result {
                Ok(Ok(saver)) => {
                    info!("Checkpoint saver: Postgres (wrapped for async compatibility)");
                    return Arc::new(BlockingPostgresSaver::new(saver));
                }
                Ok(Err(e)) => {
                    warn!("Failed to connect to Postgres for checkpoints: {e}. Falling back to in-memory.");
                }
                Err(e) => {
                    warn!("Postgres checkpoint init panicked: {e}. Falling back to in-memory.");
                }
            }
        }
        _ => {
            info!("POSTGRES_URL not set, using in-memory checkpoint saver");
        }
    }

    Arc::new(InMemorySaver::new())
}
