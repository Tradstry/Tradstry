use crate::langgraph_rs::{
    checkpoint::base::{
        ChannelVersions, Checkpoint, CheckpointConfig, CheckpointError, CheckpointMetadata,
        CheckpointSaver,
    },
    core::types::ChannelName,
};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct PersistTaskWrites {
    pub task_id: String,
    pub task_path: String,
    pub writes: Vec<(ChannelName, Value)>,
}

#[derive(Debug, Clone)]
pub struct PersistPayload {
    pub checkpoint: Checkpoint,
    pub metadata: CheckpointMetadata,
    pub new_versions: ChannelVersions,
    pub writes: Vec<PersistTaskWrites>,
    pub step: u64,
    pub source: &'static str,
}

#[derive(Debug, Clone)]
pub struct PersistResult {
    pub checkpoint_config: CheckpointConfig,
    pub checkpoint_id: String,
    pub step: u64,
    pub source: &'static str,
}

/// Persist a checkpoint payload using the saver's async APIs.
///
/// Ordering is load-bearing: the checkpoint `aput` is awaited to completion
/// before any dependent task-write `aput_writes` are issued, so that no write
/// can become durable before its checkpoint exists.
pub async fn persist_payload(
    saver: &dyn CheckpointSaver,
    checkpoint_config: &CheckpointConfig,
    payload: PersistPayload,
) -> Result<PersistResult, CheckpointError> {
    let checkpoint_id = payload.checkpoint.id.clone();
    let checkpoint_config = saver
        .aput(
            checkpoint_config,
            payload.checkpoint,
            payload.metadata,
            payload.new_versions,
        )
        .await?;

    for write in payload.writes {
        saver
            .aput_writes(
                &checkpoint_config,
                &write.writes,
                &write.task_id,
                &write.task_path,
            )
            .await?;
    }

    Ok(PersistResult {
        checkpoint_config,
        checkpoint_id,
        step: payload.step,
        source: payload.source,
    })
}
