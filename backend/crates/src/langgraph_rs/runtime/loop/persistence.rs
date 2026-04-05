use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

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

pub fn persist_payload(
    saver: &dyn CheckpointSaver,
    checkpoint_config: &CheckpointConfig,
    payload: PersistPayload,
) -> Result<PersistResult, CheckpointError> {
    let checkpoint_id = payload.checkpoint.id.clone();
    let checkpoint_config = saver.put(
        checkpoint_config,
        payload.checkpoint,
        payload.metadata,
        payload.new_versions,
    )?;

    for write in payload.writes {
        saver.put_writes(
            &checkpoint_config,
            &write.writes,
            &write.task_id,
            &write.task_path,
        )?;
    }

    Ok(PersistResult {
        checkpoint_config,
        checkpoint_id,
        step: payload.step,
        source: payload.source,
    })
}

enum AsyncPersistMessage {
    Persist {
        checkpoint_config: CheckpointConfig,
        payload: Box<PersistPayload>,
    },
    Shutdown,
}

pub struct AsyncPersistenceWorker<'scope> {
    sender: Sender<AsyncPersistMessage>,
    receiver: Receiver<Result<PersistResult, CheckpointError>>,
    handle: Option<std::thread::ScopedJoinHandle<'scope, ()>>,
    in_flight: usize,
}

impl<'scope> AsyncPersistenceWorker<'scope> {
    pub fn spawn<'env>(
        scope: &'scope std::thread::Scope<'scope, 'env>,
        saver: &'env dyn CheckpointSaver,
    ) -> Self {
        let (message_tx, message_rx) = mpsc::channel::<AsyncPersistMessage>();
        let (result_tx, result_rx) = mpsc::channel::<Result<PersistResult, CheckpointError>>();

        let handle = scope.spawn(move || {
            while let Ok(message) = message_rx.recv() {
                match message {
                    AsyncPersistMessage::Persist {
                        checkpoint_config,
                        payload,
                    } => {
                        let result = persist_payload(saver, &checkpoint_config, *payload);
                        if result_tx.send(result).is_err() {
                            break;
                        }
                    }
                    AsyncPersistMessage::Shutdown => break,
                }
            }
        });

        Self {
            sender: message_tx,
            receiver: result_rx,
            handle: Some(handle),
            in_flight: 0,
        }
    }

    pub fn enqueue(
        &mut self,
        checkpoint_config: CheckpointConfig,
        payload: PersistPayload,
    ) -> Result<(), CheckpointError> {
        self.sender
            .send(AsyncPersistMessage::Persist {
                checkpoint_config,
                payload: Box::new(payload),
            })
            .map_err(|_| CheckpointError::storage("async persistence worker channel closed"))?;
        self.in_flight = self.in_flight.saturating_add(1);
        Ok(())
    }

    pub fn drain_ready(&mut self) -> Vec<Result<PersistResult, CheckpointError>> {
        let mut drained = Vec::new();
        loop {
            match self.receiver.try_recv() {
                Ok(result) => {
                    self.in_flight = self.in_flight.saturating_sub(1);
                    drained.push(result);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if self.in_flight > 0 {
                        self.in_flight = 0;
                        drained.push(Err(CheckpointError::storage(
                            "async persistence worker disconnected unexpectedly",
                        )));
                    }
                    break;
                }
            }
        }
        drained
    }

    pub fn finish(mut self) -> Vec<Result<PersistResult, CheckpointError>> {
        let mut drained = self.drain_ready();
        let _ = self.sender.send(AsyncPersistMessage::Shutdown);

        if let Some(handle) = self.handle.take()
            && handle.join().is_err()
        {
            drained.push(Err(CheckpointError::storage(
                "async persistence worker panicked",
            )));
        }

        while self.in_flight > 0 {
            match self.receiver.recv() {
                Ok(result) => {
                    self.in_flight = self.in_flight.saturating_sub(1);
                    drained.push(result);
                }
                Err(_) => {
                    self.in_flight = 0;
                    drained.push(Err(CheckpointError::storage(
                        "async persistence worker terminated before all payloads were flushed",
                    )));
                }
            }
        }

        drained
    }
}
