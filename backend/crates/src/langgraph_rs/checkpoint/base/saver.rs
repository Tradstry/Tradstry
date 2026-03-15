use async_trait::async_trait;
use serde_json::Value;

use crate::langgraph_rs::core::types::{ChannelName, TaskId};

use super::{
    ChannelVersions, Checkpoint, CheckpointConfig, CheckpointError, CheckpointMetadata,
    CheckpointTuple, ListCheckpointsQuery, PruneStrategy,
};

#[async_trait]
pub trait CheckpointSaver: Send + Sync {
    fn get(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, CheckpointError> {
        Ok(self.get_tuple(config)?.map(|tuple| tuple.checkpoint))
    }

    fn get_tuple(
        &self,
        _config: &CheckpointConfig,
    ) -> Result<Option<CheckpointTuple>, CheckpointError> {
        Err(CheckpointError::not_implemented("get_tuple"))
    }

    fn list(&self, _query: &ListCheckpointsQuery) -> Result<Vec<CheckpointTuple>, CheckpointError> {
        Err(CheckpointError::not_implemented("list"))
    }

    fn put(
        &self,
        _config: &CheckpointConfig,
        _checkpoint: Checkpoint,
        _metadata: CheckpointMetadata,
        _new_versions: ChannelVersions,
    ) -> Result<CheckpointConfig, CheckpointError> {
        Err(CheckpointError::not_implemented("put"))
    }

    fn put_writes(
        &self,
        _config: &CheckpointConfig,
        _writes: &[(ChannelName, Value)],
        _task_id: &TaskId,
        _task_path: &str,
    ) -> Result<(), CheckpointError> {
        Err(CheckpointError::not_implemented("put_writes"))
    }

    fn delete_thread(&self, _thread_id: &str) -> Result<(), CheckpointError> {
        Err(CheckpointError::not_implemented("delete_thread"))
    }

    fn delete_for_runs(&self, _run_ids: &[String]) -> Result<(), CheckpointError> {
        Err(CheckpointError::unsupported_capability("delete_for_runs"))
    }

    fn copy_thread(
        &self,
        _source_thread_id: &str,
        _target_thread_id: &str,
    ) -> Result<(), CheckpointError> {
        Err(CheckpointError::unsupported_capability("copy_thread"))
    }

    fn prune(
        &self,
        _thread_ids: &[String],
        _strategy: PruneStrategy,
    ) -> Result<(), CheckpointError> {
        Err(CheckpointError::unsupported_capability("prune"))
    }

    async fn aget(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, CheckpointError> {
        self.get(config)
    }

    async fn aget_tuple(
        &self,
        config: &CheckpointConfig,
    ) -> Result<Option<CheckpointTuple>, CheckpointError> {
        self.get_tuple(config)
    }

    async fn alist(
        &self,
        query: &ListCheckpointsQuery,
    ) -> Result<Vec<CheckpointTuple>, CheckpointError> {
        self.list(query)
    }

    async fn aput(
        &self,
        config: &CheckpointConfig,
        checkpoint: Checkpoint,
        metadata: CheckpointMetadata,
        new_versions: ChannelVersions,
    ) -> Result<CheckpointConfig, CheckpointError> {
        self.put(config, checkpoint, metadata, new_versions)
    }

    async fn aput_writes(
        &self,
        config: &CheckpointConfig,
        writes: &[(ChannelName, Value)],
        task_id: &TaskId,
        task_path: &str,
    ) -> Result<(), CheckpointError> {
        self.put_writes(config, writes, task_id, task_path)
    }

    async fn adelete_thread(&self, thread_id: &str) -> Result<(), CheckpointError> {
        self.delete_thread(thread_id)
    }

    async fn adelete_for_runs(&self, run_ids: &[String]) -> Result<(), CheckpointError> {
        self.delete_for_runs(run_ids)
    }

    async fn acopy_thread(
        &self,
        source_thread_id: &str,
        target_thread_id: &str,
    ) -> Result<(), CheckpointError> {
        self.copy_thread(source_thread_id, target_thread_id)
    }

    async fn aprune(
        &self,
        thread_ids: &[String],
        strategy: PruneStrategy,
    ) -> Result<(), CheckpointError> {
        self.prune(thread_ids, strategy)
    }

    fn get_next_version(&self, current: Option<u64>) -> u64 {
        current.unwrap_or(0).saturating_add(1)
    }
}

#[cfg(test)]
mod tests {
    use super::CheckpointSaver;

    struct NoopSaver;

    impl CheckpointSaver for NoopSaver {}

    #[test]
    fn default_next_version_is_incremental() {
        let saver = NoopSaver;
        assert_eq!(saver.get_next_version(None), 1);
        assert_eq!(saver.get_next_version(Some(7)), 8);
    }
}
