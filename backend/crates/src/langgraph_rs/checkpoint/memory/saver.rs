use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use serde_json::Value;

use crate::langgraph_rs::{
    checkpoint::base::{
        ChannelVersions, Checkpoint, CheckpointConfig, CheckpointError, CheckpointMetadata,
        CheckpointSaver, CheckpointSource, CheckpointTuple, ListCheckpointsQuery, PendingWrite,
        PruneStrategy, get_serializable_checkpoint_metadata, normalize_checkpoint_for_read,
        project_checkpoint_for_storage, write_idx_for_channel,
    },
    core::types::{ChannelName, TaskId},
};

#[derive(Debug, Clone)]
struct StoredCheckpoint {
    checkpoint: Checkpoint,
    metadata: CheckpointMetadata,
    parent_checkpoint_id: Option<String>,
}

type WritesOuterKey = (String, String, String);
type WritesInnerKey = (TaskId, i32);

#[derive(Debug, Default)]
struct InMemoryState {
    storage: BTreeMap<String, BTreeMap<String, BTreeMap<String, StoredCheckpoint>>>,
    writes: BTreeMap<WritesOuterKey, BTreeMap<WritesInnerKey, PendingWrite>>,
}

#[derive(Debug, Default)]
pub struct InMemorySaver {
    state: RwLock<InMemoryState>,
}

impl InMemorySaver {
    pub fn new() -> Self {
        Self::default()
    }

    fn read_state(&self) -> Result<RwLockReadGuard<'_, InMemoryState>, CheckpointError> {
        self.state
            .read()
            .map_err(|_| CheckpointError::storage("in-memory saver read lock poisoned"))
    }

    fn write_state(&self) -> Result<RwLockWriteGuard<'_, InMemoryState>, CheckpointError> {
        self.state
            .write()
            .map_err(|_| CheckpointError::storage("in-memory saver write lock poisoned"))
    }
}

impl CheckpointSaver for InMemorySaver {
    fn get_tuple(
        &self,
        config: &CheckpointConfig,
    ) -> Result<Option<CheckpointTuple>, CheckpointError> {
        let state = self.read_state()?;

        let Some(by_namespace) = state.storage.get(&config.thread_id) else {
            return Ok(None);
        };
        let Some(checkpoints) = by_namespace.get(&config.checkpoint_ns) else {
            return Ok(None);
        };

        let checkpoint_id = match &config.checkpoint_id {
            Some(checkpoint_id) => checkpoint_id.clone(),
            None => match checkpoints.keys().next_back() {
                Some(latest) => latest.clone(),
                None => return Ok(None),
            },
        };

        let Some(stored) = checkpoints.get(&checkpoint_id) else {
            return Ok(None);
        };

        let writes_key = (
            config.thread_id.clone(),
            config.checkpoint_ns.clone(),
            checkpoint_id.clone(),
        );

        let pending_writes = state
            .writes
            .get(&writes_key)
            .map(|writes| writes.values().cloned().collect::<Vec<_>>())
            .filter(|writes| !writes.is_empty());

        let checkpoint = normalize_checkpoint_for_read(stored.checkpoint.clone(), config)?;
        let mut tuple_config = config.clone();
        tuple_config.checkpoint_id = Some(checkpoint_id.clone());

        Ok(Some(CheckpointTuple {
            config: tuple_config,
            checkpoint,
            metadata: stored.metadata.clone(),
            parent_config: stored
                .parent_checkpoint_id
                .as_ref()
                .map(|parent_checkpoint_id| {
                    let mut parent = config.clone();
                    parent.checkpoint_id = Some(parent_checkpoint_id.clone());
                    parent
                }),
            pending_writes,
        }))
    }

    fn list(&self, query: &ListCheckpointsQuery) -> Result<Vec<CheckpointTuple>, CheckpointError> {
        let state = self.read_state()?;
        let mut tuples = Vec::new();
        let mut remaining = query.limit.unwrap_or(usize::MAX);

        for (thread_id, by_namespace) in &state.storage {
            if let Some(config) = &query.config {
                if thread_id != &config.thread_id {
                    continue;
                }
            }

            for (checkpoint_ns, checkpoints) in by_namespace {
                if let Some(config) = &query.config {
                    if checkpoint_ns != &config.checkpoint_ns {
                        continue;
                    }
                }

                for (checkpoint_id, stored) in checkpoints.iter().rev() {
                    if remaining == 0 {
                        return Ok(tuples);
                    }

                    if let Some(config) = &query.config {
                        if let Some(config_checkpoint_id) = &config.checkpoint_id {
                            if checkpoint_id != config_checkpoint_id {
                                continue;
                            }
                        }
                    }

                    if let Some(before) = &query.before {
                        if before.thread_id == *thread_id
                            && before.checkpoint_ns == *checkpoint_ns
                            && before
                                .checkpoint_id
                                .as_ref()
                                .is_some_and(|before_id| checkpoint_id >= before_id)
                        {
                            continue;
                        }
                    }

                    if !metadata_matches(&query.metadata_filter, &stored.metadata) {
                        continue;
                    }

                    let writes_key = (
                        thread_id.clone(),
                        checkpoint_ns.clone(),
                        checkpoint_id.clone(),
                    );
                    let pending_writes = state
                        .writes
                        .get(&writes_key)
                        .map(|writes| writes.values().cloned().collect::<Vec<_>>())
                        .filter(|writes| !writes.is_empty());

                    let mut tuple_config = query.config.clone().unwrap_or_else(|| {
                        CheckpointConfig::new(thread_id.clone())
                            .with_namespace(checkpoint_ns.clone())
                    });
                    tuple_config.thread_id = thread_id.clone();
                    tuple_config.checkpoint_ns = checkpoint_ns.clone();
                    tuple_config.checkpoint_id = Some(checkpoint_id.clone());
                    let checkpoint =
                        normalize_checkpoint_for_read(stored.checkpoint.clone(), &tuple_config)?;

                    tuples.push(CheckpointTuple {
                        config: tuple_config,
                        checkpoint,
                        metadata: stored.metadata.clone(),
                        parent_config: stored.parent_checkpoint_id.as_ref().map(
                            |parent_checkpoint_id| {
                                let mut parent = CheckpointConfig::new(thread_id.clone())
                                    .with_namespace(checkpoint_ns.clone());
                                parent.checkpoint_id = Some(parent_checkpoint_id.clone());
                                parent
                            },
                        ),
                        pending_writes,
                    });

                    remaining = remaining.saturating_sub(1);
                }
            }
        }

        Ok(tuples)
    }

    fn put(
        &self,
        config: &CheckpointConfig,
        checkpoint: Checkpoint,
        metadata: CheckpointMetadata,
        new_versions: ChannelVersions,
    ) -> Result<CheckpointConfig, CheckpointError> {
        if config.thread_id.is_empty() {
            return Err(CheckpointError::invalid_config(
                "thread_id cannot be empty for put",
            ));
        }

        let thread_id = config.thread_id.clone();
        let checkpoint_ns = config.checkpoint_ns.clone();
        let parent_checkpoint_id = config.checkpoint_id.clone();
        let mut checkpoint = project_checkpoint_for_storage(checkpoint, config);
        let checkpoint_id = checkpoint.id.clone();
        let metadata = get_serializable_checkpoint_metadata(config, &metadata);

        for (channel_name, version) in new_versions {
            checkpoint.channel_versions.insert(channel_name, version);
        }

        let mut state = self.write_state()?;
        state
            .storage
            .entry(thread_id.clone())
            .or_default()
            .entry(checkpoint_ns.clone())
            .or_default()
            .insert(
                checkpoint_id.clone(),
                StoredCheckpoint {
                    checkpoint,
                    metadata,
                    parent_checkpoint_id,
                },
            );

        let mut next_config = config.clone();
        next_config.thread_id = thread_id;
        next_config.checkpoint_ns = checkpoint_ns;
        next_config.checkpoint_id = Some(checkpoint_id);
        Ok(next_config)
    }

    fn put_writes(
        &self,
        config: &CheckpointConfig,
        writes: &[(ChannelName, Value)],
        task_id: &TaskId,
        task_path: &str,
    ) -> Result<(), CheckpointError> {
        let checkpoint_id = config.checkpoint_id.as_ref().ok_or_else(|| {
            CheckpointError::invalid_config("checkpoint_id is required for put_writes")
        })?;

        let outer_key = (
            config.thread_id.clone(),
            config.checkpoint_ns.clone(),
            checkpoint_id.clone(),
        );

        let mut state = self.write_state()?;
        let outer_writes = state.writes.entry(outer_key).or_default();

        for (idx, (channel_name, value)) in writes.iter().enumerate() {
            let write_idx = write_idx_for_channel(channel_name, idx);
            let inner_key = (task_id.clone(), write_idx);

            if write_idx >= 0 && outer_writes.contains_key(&inner_key) {
                continue;
            }

            outer_writes.insert(
                inner_key,
                PendingWrite::new(task_id.clone(), channel_name.clone(), value.clone())
                    .with_task_path(task_path),
            );
        }

        Ok(())
    }

    fn delete_thread(&self, thread_id: &str) -> Result<(), CheckpointError> {
        let mut state = self.write_state()?;
        state.storage.remove(thread_id);
        state
            .writes
            .retain(|(stored_thread_id, _, _), _| stored_thread_id != thread_id);
        Ok(())
    }

    fn delete_for_runs(&self, run_ids: &[String]) -> Result<(), CheckpointError> {
        if run_ids.is_empty() {
            return Ok(());
        }

        let run_ids: BTreeSet<&str> = run_ids.iter().map(String::as_str).collect();
        let mut state = self.write_state()?;
        let mut removed_writes = Vec::<WritesOuterKey>::new();

        for (thread_id, by_namespace) in &mut state.storage {
            for (checkpoint_ns, checkpoints) in by_namespace.iter_mut() {
                checkpoints.retain(|checkpoint_id, stored| {
                    let should_remove = stored
                        .metadata
                        .run_id
                        .as_deref()
                        .is_some_and(|run_id| run_ids.contains(run_id));

                    if should_remove {
                        removed_writes.push((
                            thread_id.clone(),
                            checkpoint_ns.clone(),
                            checkpoint_id.clone(),
                        ));
                    }

                    !should_remove
                });
            }

            by_namespace.retain(|_, checkpoints| !checkpoints.is_empty());
        }

        state
            .storage
            .retain(|_, by_namespace| !by_namespace.is_empty());

        for write_key in removed_writes {
            state.writes.remove(&write_key);
        }

        Ok(())
    }

    fn copy_thread(
        &self,
        source_thread_id: &str,
        target_thread_id: &str,
    ) -> Result<(), CheckpointError> {
        if source_thread_id == target_thread_id {
            return Ok(());
        }

        let mut state = self.write_state()?;
        let Some(source_storage) = state.storage.get(source_thread_id).cloned() else {
            return Ok(());
        };

        state
            .writes
            .retain(|(thread_id, _, _), _| thread_id != target_thread_id);

        let source_writes: Vec<(WritesOuterKey, BTreeMap<WritesInnerKey, PendingWrite>)> = state
            .writes
            .iter()
            .filter(|((thread_id, _, _), _)| thread_id == source_thread_id)
            .map(|((_, checkpoint_ns, checkpoint_id), writes)| {
                (
                    (
                        target_thread_id.to_owned(),
                        checkpoint_ns.clone(),
                        checkpoint_id.clone(),
                    ),
                    writes.clone(),
                )
            })
            .collect();

        state
            .storage
            .insert(target_thread_id.to_owned(), source_storage);

        for (writes_key, writes) in source_writes {
            state.writes.insert(writes_key, writes);
        }

        Ok(())
    }

    fn prune(&self, thread_ids: &[String], strategy: PruneStrategy) -> Result<(), CheckpointError> {
        let mut state = self.write_state()?;

        match strategy {
            PruneStrategy::Delete => {
                let threads: BTreeSet<&str> = thread_ids.iter().map(String::as_str).collect();
                for thread_id in &threads {
                    state.storage.remove(*thread_id);
                }
                state
                    .writes
                    .retain(|(thread_id, _, _), _| !threads.contains(thread_id.as_str()));
                Ok(())
            }
            PruneStrategy::KeepLatest => {
                let affected_threads: BTreeSet<&str> =
                    thread_ids.iter().map(String::as_str).collect();

                for thread_id in &affected_threads {
                    if let Some(by_namespace) = state.storage.get_mut(*thread_id) {
                        for checkpoints in by_namespace.values_mut() {
                            let Some(latest_checkpoint_id) =
                                checkpoints.keys().next_back().cloned()
                            else {
                                continue;
                            };
                            checkpoints
                                .retain(|checkpoint_id, _| checkpoint_id == &latest_checkpoint_id);
                        }
                        by_namespace.retain(|_, checkpoints| !checkpoints.is_empty());
                    }
                }

                let mut keep_writes = BTreeSet::<WritesOuterKey>::new();
                for thread_id in &affected_threads {
                    if let Some(by_namespace) = state.storage.get(*thread_id) {
                        for (checkpoint_ns, checkpoints) in by_namespace {
                            for checkpoint_id in checkpoints.keys() {
                                keep_writes.insert((
                                    (*thread_id).to_owned(),
                                    checkpoint_ns.clone(),
                                    checkpoint_id.clone(),
                                ));
                            }
                        }
                    }
                }

                state.writes.retain(|write_key, _| {
                    if !affected_threads.contains(write_key.0.as_str()) {
                        return true;
                    }
                    keep_writes.contains(write_key)
                });

                state
                    .storage
                    .retain(|_, by_namespace| !by_namespace.is_empty());
                Ok(())
            }
        }
    }
}

fn metadata_matches(filter: &BTreeMap<String, Value>, metadata: &CheckpointMetadata) -> bool {
    if filter.is_empty() {
        return true;
    }

    filter.iter().all(|(key, expected_value)| {
        metadata_field_value(metadata, key)
            .as_ref()
            .is_some_and(|actual_value| actual_value == expected_value)
    })
}

fn metadata_field_value(metadata: &CheckpointMetadata, key: &str) -> Option<Value> {
    match key {
        "source" => metadata
            .source
            .map(|source| Value::String(checkpoint_source_str(source).to_owned())),
        "step" => metadata.step.map(Value::from),
        "run_id" => metadata.run_id.clone().map(Value::String),
        "parents" => Some(Value::Object(
            metadata
                .parents
                .iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect(),
        )),
        _ => metadata.extra.get(key).cloned(),
    }
}

fn checkpoint_source_str(source: CheckpointSource) -> &'static str {
    match source {
        CheckpointSource::Input => "input",
        CheckpointSource::Loop => "loop",
        CheckpointSource::Update => "update",
        CheckpointSource::Fork => "fork",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::langgraph_rs::checkpoint::{
        base::{
            Checkpoint, CheckpointConfig, CheckpointMetadata, CheckpointSaver, CheckpointSource,
            ListCheckpointsQuery, PruneStrategy,
        },
        memory::InMemorySaver,
    };

    fn checkpoint(id: &str, value: i64) -> Checkpoint {
        let mut checkpoint = Checkpoint::new(id, format!("{id}.ts"));
        checkpoint
            .channel_values
            .insert("messages".to_owned(), json!(value));
        checkpoint
            .channel_versions
            .insert("messages".to_owned(), value as u64);
        checkpoint
    }

    #[test]
    fn put_and_get_tuple_returns_latest_checkpoint() {
        let saver = InMemorySaver::new();

        let base_config = CheckpointConfig::new("thread-1");
        let first = saver
            .put(
                &base_config,
                checkpoint("0001", 1),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();
        let second = saver
            .put(
                &first,
                checkpoint("0002", 2),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();

        let latest = saver.get_tuple(&base_config).unwrap().unwrap();

        assert_eq!(latest.checkpoint.id, "0002");
        assert_eq!(
            latest
                .parent_config
                .as_ref()
                .and_then(|config| config.checkpoint_id.clone()),
            Some("0001".to_owned())
        );
        assert_eq!(second.checkpoint_id, Some("0002".to_owned()));
    }

    #[test]
    fn put_writes_dedupes_regular_indices_and_overwrites_special_channels() {
        let saver = InMemorySaver::new();

        let config = saver
            .put(
                &CheckpointConfig::new("thread-1"),
                checkpoint("0001", 1),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();

        saver
            .put_writes(
                &config,
                &[
                    ("messages".to_owned(), json!(1)),
                    ("__error__".to_owned(), json!("e1")),
                ],
                &"task-1".to_owned(),
                "path",
            )
            .unwrap();

        saver
            .put_writes(
                &config,
                &[
                    ("messages".to_owned(), json!(999)),
                    ("__error__".to_owned(), json!("e2")),
                ],
                &"task-1".to_owned(),
                "path",
            )
            .unwrap();

        let tuple = saver.get_tuple(&config).unwrap().unwrap();
        let writes = tuple.pending_writes.unwrap();

        assert_eq!(writes.len(), 2);
        assert!(
            writes
                .iter()
                .any(|write| write.channel == "messages" && write.value == json!(1))
        );
        assert!(
            writes
                .iter()
                .any(|write| write.channel == "__error__" && write.value == json!("e2"))
        );
    }

    #[test]
    fn list_applies_before_and_limit_filters() {
        let saver = InMemorySaver::new();

        let base = CheckpointConfig::new("thread-1");
        let c1 = saver
            .put(
                &base,
                checkpoint("0001", 1),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();
        let c2 = saver
            .put(
                &c1,
                checkpoint("0002", 2),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();
        saver
            .put(
                &c2,
                checkpoint("0003", 3),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();

        let query = ListCheckpointsQuery {
            config: Some(CheckpointConfig::new("thread-1")),
            metadata_filter: BTreeMap::new(),
            before: Some(CheckpointConfig::new("thread-1").with_checkpoint_id("0003")),
            limit: Some(1),
        };

        let listed = saver.list(&query).unwrap();

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].checkpoint.id, "0002");
    }

    #[test]
    fn delete_thread_removes_checkpoints_and_writes() {
        let saver = InMemorySaver::new();

        let config = saver
            .put(
                &CheckpointConfig::new("thread-1"),
                checkpoint("0001", 1),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();
        saver
            .put_writes(
                &config,
                &[("messages".to_owned(), json!(1))],
                &"task-1".to_owned(),
                "",
            )
            .unwrap();

        saver.delete_thread("thread-1").unwrap();

        assert!(
            saver
                .get_tuple(&CheckpointConfig::new("thread-1"))
                .unwrap()
                .is_none()
        );
        assert!(
            saver
                .list(&ListCheckpointsQuery {
                    config: Some(CheckpointConfig::new("thread-1")),
                    ..Default::default()
                })
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn copy_thread_clones_checkpoints_and_writes() {
        let saver = InMemorySaver::new();

        let source_config = saver
            .put(
                &CheckpointConfig::new("source"),
                checkpoint("0001", 1),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();
        saver
            .put_writes(
                &source_config,
                &[("messages".to_owned(), json!("copied"))],
                &"task-1".to_owned(),
                "",
            )
            .unwrap();

        saver.copy_thread("source", "target").unwrap();

        let target_tuple = saver
            .get_tuple(&CheckpointConfig::new("target"))
            .unwrap()
            .unwrap();
        assert_eq!(target_tuple.checkpoint.id, "0001");
        assert!(
            target_tuple
                .pending_writes
                .unwrap_or_default()
                .iter()
                .any(|write| write.channel == "messages" && write.value == json!("copied"))
        );
    }

    #[test]
    fn prune_keep_latest_keeps_latest_per_namespace() {
        let saver = InMemorySaver::new();

        let base = CheckpointConfig::new("thread-1");
        let a1 = saver
            .put(
                &base,
                checkpoint("0001", 1),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();
        saver
            .put(
                &a1,
                checkpoint("0002", 2),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();

        let ns_config = CheckpointConfig::new("thread-1").with_namespace("chat");
        let b1 = saver
            .put(
                &ns_config,
                checkpoint("1001", 10),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();
        saver
            .put(
                &b1,
                checkpoint("1002", 20),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();

        saver
            .prune(&["thread-1".to_owned()], PruneStrategy::KeepLatest)
            .unwrap();

        let default_ns = saver
            .list(&ListCheckpointsQuery {
                config: Some(CheckpointConfig::new("thread-1")),
                ..Default::default()
            })
            .unwrap();
        let chat_ns = saver
            .list(&ListCheckpointsQuery {
                config: Some(CheckpointConfig::new("thread-1").with_namespace("chat")),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(default_ns.len(), 1);
        assert_eq!(default_ns[0].checkpoint.id, "0002");
        assert_eq!(chat_ns.len(), 1);
        assert_eq!(chat_ns[0].checkpoint.id, "1002");
    }

    #[test]
    fn delete_for_runs_removes_matching_run_ids_only() {
        let saver = InMemorySaver::new();

        let mut remove_md = CheckpointMetadata {
            source: Some(CheckpointSource::Loop),
            step: Some(1),
            parents: BTreeMap::new(),
            run_id: Some("run-remove".to_owned()),
            extra: BTreeMap::new(),
        };
        remove_md.extra.insert("kind".to_owned(), json!("remove"));

        let keep_md = CheckpointMetadata {
            source: Some(CheckpointSource::Loop),
            step: Some(2),
            parents: BTreeMap::new(),
            run_id: Some("run-keep".to_owned()),
            extra: BTreeMap::new(),
        };

        saver
            .put(
                &CheckpointConfig::new("thread-1"),
                checkpoint("0001", 1),
                remove_md,
                BTreeMap::new(),
            )
            .unwrap();
        saver
            .put(
                &CheckpointConfig::new("thread-1"),
                checkpoint("0002", 2),
                keep_md,
                BTreeMap::new(),
            )
            .unwrap();

        saver.delete_for_runs(&["run-remove".to_owned()]).unwrap();

        let remaining = saver
            .list(&ListCheckpointsQuery {
                config: Some(CheckpointConfig::new("thread-1")),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].checkpoint.id, "0002");
    }
}
