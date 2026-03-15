use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::langgraph_rs::{
    checkpoint::base::{
        ChannelVersions, Checkpoint, CheckpointConfig, CheckpointError, CheckpointMetadata,
        CheckpointSaver, CheckpointSource, CheckpointTuple, ListCheckpointsQuery, PendingWrite,
        PruneStrategy, deserialize_checkpoint_json, get_serializable_checkpoint_metadata,
        project_checkpoint_for_storage, write_idx_for_channel,
    },
    core::types::{ChannelName, TaskId},
};

#[derive(Debug, Clone)]
pub struct SqliteSaver {
    db_path: PathBuf,
}

impl SqliteSaver {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CheckpointError> {
        let db_path = path.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| {
                    CheckpointError::storage(format!(
                        "failed to create sqlite parent directory '{}': {err}",
                        parent.display()
                    ))
                })?;
            }
        }

        let saver = Self { db_path };
        let conn = saver.open_connection()?;
        Self::initialize_schema(&conn)?;
        Ok(saver)
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }

    fn open_connection(&self) -> Result<Connection, CheckpointError> {
        let conn = Connection::open(&self.db_path).map_err(|err| {
            CheckpointError::storage(format!(
                "failed to open sqlite db '{}': {err}",
                self.db_path.display()
            ))
        })?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(|err| {
                CheckpointError::storage(format!("failed to enable foreign keys: {err}"))
            })?;
        Self::initialize_schema(&conn)?;
        Ok(conn)
    }

    fn initialize_schema(conn: &Connection) -> Result<(), CheckpointError> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS checkpoints (
                thread_id TEXT NOT NULL,
                checkpoint_ns TEXT NOT NULL,
                checkpoint_id TEXT NOT NULL,
                parent_checkpoint_id TEXT,
                ts TEXT NOT NULL,
                run_id TEXT,
                source TEXT,
                step INTEGER,
                checkpoint_json TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                PRIMARY KEY (thread_id, checkpoint_ns, checkpoint_id)
            );

            CREATE INDEX IF NOT EXISTS idx_checkpoints_thread_ns_id
                ON checkpoints (thread_id, checkpoint_ns, checkpoint_id DESC);

            CREATE INDEX IF NOT EXISTS idx_checkpoints_run_id
                ON checkpoints (run_id);

            CREATE TABLE IF NOT EXISTS writes (
                thread_id TEXT NOT NULL,
                checkpoint_ns TEXT NOT NULL,
                checkpoint_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                write_idx INTEGER NOT NULL,
                channel TEXT NOT NULL,
                value_json TEXT NOT NULL,
                task_path TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (thread_id, checkpoint_ns, checkpoint_id, task_id, write_idx),
                FOREIGN KEY (thread_id, checkpoint_ns, checkpoint_id)
                    REFERENCES checkpoints (thread_id, checkpoint_ns, checkpoint_id)
                    ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_writes_lookup
                ON writes (thread_id, checkpoint_ns, checkpoint_id, task_id, write_idx);
            "#,
        )
        .map_err(|err| {
            CheckpointError::storage(format!("failed to initialize sqlite schema: {err}"))
        })
    }

    fn load_pending_writes(
        conn: &Connection,
        thread_id: &str,
        checkpoint_ns: &str,
        checkpoint_id: &str,
    ) -> Result<Option<Vec<PendingWrite>>, CheckpointError> {
        let mut stmt = conn
            .prepare(
                r#"
                SELECT task_id, channel, value_json, task_path
                FROM writes
                WHERE thread_id = ?1
                  AND checkpoint_ns = ?2
                  AND checkpoint_id = ?3
                ORDER BY task_id ASC, write_idx ASC
                "#,
            )
            .map_err(|err| {
                CheckpointError::storage(format!("failed to prepare writes query: {err}"))
            })?;

        let mut rows = stmt
            .query(params![thread_id, checkpoint_ns, checkpoint_id])
            .map_err(|err| CheckpointError::storage(format!("failed to query writes: {err}")))?;

        let mut pending_writes = Vec::new();
        while let Some(row) = rows
            .next()
            .map_err(|err| CheckpointError::storage(format!("failed to iterate writes: {err}")))?
        {
            let task_id: String = row.get(0).map_err(|err| {
                CheckpointError::storage(format!("failed to read task_id: {err}"))
            })?;
            let channel: String = row.get(1).map_err(|err| {
                CheckpointError::storage(format!("failed to read channel: {err}"))
            })?;
            let value_json: String = row.get(2).map_err(|err| {
                CheckpointError::storage(format!("failed to read write value: {err}"))
            })?;
            let task_path: String = row.get(3).map_err(|err| {
                CheckpointError::storage(format!("failed to read task_path: {err}"))
            })?;

            let value = serde_json::from_str(&value_json).map_err(|err| {
                CheckpointError::serialization(format!(
                    "failed to deserialize write value for task '{task_id}': {err}"
                ))
            })?;

            pending_writes.push(PendingWrite {
                task_id,
                channel,
                value,
                task_path,
            });
        }

        if pending_writes.is_empty() {
            Ok(None)
        } else {
            Ok(Some(pending_writes))
        }
    }
}

impl CheckpointSaver for SqliteSaver {
    fn get_tuple(
        &self,
        config: &CheckpointConfig,
    ) -> Result<Option<CheckpointTuple>, CheckpointError> {
        if config.thread_id.is_empty() {
            return Err(CheckpointError::invalid_config(
                "thread_id cannot be empty for get_tuple",
            ));
        }

        let conn = self.open_connection()?;
        let row: Option<(String, String, String, Option<String>)> = if let Some(checkpoint_id) =
            &config.checkpoint_id
        {
            conn.query_row(
                r#"
                SELECT checkpoint_id, checkpoint_json, metadata_json, parent_checkpoint_id
                FROM checkpoints
                WHERE thread_id = ?1
                  AND checkpoint_ns = ?2
                  AND checkpoint_id = ?3
                "#,
                params![&config.thread_id, &config.checkpoint_ns, checkpoint_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|err| {
                CheckpointError::storage(format!("failed to fetch checkpoint tuple: {err}"))
            })?
        } else {
            conn.query_row(
                r#"
                SELECT checkpoint_id, checkpoint_json, metadata_json, parent_checkpoint_id
                FROM checkpoints
                WHERE thread_id = ?1
                  AND checkpoint_ns = ?2
                ORDER BY ts DESC, checkpoint_id DESC
                LIMIT 1
                "#,
                params![&config.thread_id, &config.checkpoint_ns],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|err| {
                CheckpointError::storage(format!("failed to fetch latest checkpoint tuple: {err}"))
            })?
        };

        let Some((checkpoint_id, checkpoint_json, metadata_json, parent_checkpoint_id)) = row
        else {
            return Ok(None);
        };

        let mut tuple_config = config.clone();
        tuple_config.checkpoint_id = Some(checkpoint_id.clone());
        let checkpoint =
            deserialize_checkpoint_json(&checkpoint_json, &tuple_config).map_err(|err| {
                CheckpointError::serialization(format!(
                    "failed to deserialize checkpoint '{checkpoint_id}': {err}"
                ))
            })?;
        let metadata: CheckpointMetadata = serde_json::from_str(&metadata_json).map_err(|err| {
            CheckpointError::serialization(format!(
                "failed to deserialize checkpoint metadata '{checkpoint_id}': {err}"
            ))
        })?;
        let pending_writes = Self::load_pending_writes(
            &conn,
            &config.thread_id,
            &config.checkpoint_ns,
            &checkpoint_id,
        )?;

        Ok(Some(CheckpointTuple {
            config: tuple_config.clone(),
            checkpoint,
            metadata,
            parent_config: parent_checkpoint_id.map(|parent_checkpoint_id| {
                let mut parent = tuple_config.clone();
                parent.checkpoint_id = Some(parent_checkpoint_id);
                parent
            }),
            pending_writes,
        }))
    }

    fn list(&self, query: &ListCheckpointsQuery) -> Result<Vec<CheckpointTuple>, CheckpointError> {
        let conn = self.open_connection()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT thread_id, checkpoint_ns, checkpoint_id, checkpoint_json, metadata_json, parent_checkpoint_id
                FROM checkpoints
                ORDER BY thread_id ASC, checkpoint_ns ASC, ts DESC, checkpoint_id DESC
                "#,
            )
            .map_err(|err| CheckpointError::storage(format!("failed to prepare checkpoint list query: {err}")))?;

        let mut rows = stmt.query([]).map_err(|err| {
            CheckpointError::storage(format!("failed to query checkpoint list: {err}"))
        })?;

        let mut tuples = Vec::new();
        let mut remaining = query.limit.unwrap_or(usize::MAX);

        while let Some(row) = rows.next().map_err(|err| {
            CheckpointError::storage(format!("failed to iterate checkpoint rows: {err}"))
        })? {
            if remaining == 0 {
                break;
            }

            let thread_id: String = row.get(0).map_err(|err| {
                CheckpointError::storage(format!("failed to read thread_id: {err}"))
            })?;
            let checkpoint_ns: String = row.get(1).map_err(|err| {
                CheckpointError::storage(format!("failed to read checkpoint_ns: {err}"))
            })?;
            let checkpoint_id: String = row.get(2).map_err(|err| {
                CheckpointError::storage(format!("failed to read checkpoint_id: {err}"))
            })?;

            if !config_matches(
                query.config.as_ref(),
                &thread_id,
                &checkpoint_ns,
                &checkpoint_id,
            ) {
                continue;
            }
            if before_filter_blocks(
                query.before.as_ref(),
                &thread_id,
                &checkpoint_ns,
                &checkpoint_id,
            ) {
                continue;
            }

            let checkpoint_json: String = row.get(3).map_err(|err| {
                CheckpointError::storage(format!("failed to read checkpoint payload: {err}"))
            })?;
            let metadata_json: String = row.get(4).map_err(|err| {
                CheckpointError::storage(format!("failed to read metadata payload: {err}"))
            })?;
            let parent_checkpoint_id: Option<String> = row.get(5).map_err(|err| {
                CheckpointError::storage(format!("failed to read parent checkpoint id: {err}"))
            })?;

            let mut tuple_config = query
                .config
                .clone()
                .unwrap_or_else(|| CheckpointConfig::new(thread_id.clone()));
            tuple_config.thread_id = thread_id.clone();
            tuple_config.checkpoint_ns = checkpoint_ns.clone();
            tuple_config.checkpoint_id = Some(checkpoint_id.clone());

            let checkpoint =
                deserialize_checkpoint_json(&checkpoint_json, &tuple_config).map_err(|err| {
                    CheckpointError::serialization(format!(
                        "failed to deserialize checkpoint '{checkpoint_id}' during list: {err}"
                    ))
                })?;
            let metadata: CheckpointMetadata =
                serde_json::from_str(&metadata_json).map_err(|err| {
                    CheckpointError::serialization(format!(
                        "failed to deserialize metadata '{checkpoint_id}' during list: {err}"
                    ))
                })?;

            if !metadata_matches(&query.metadata_filter, &metadata) {
                continue;
            }

            let pending_writes =
                Self::load_pending_writes(&conn, &thread_id, &checkpoint_ns, &checkpoint_id)?;

            tuples.push(CheckpointTuple {
                config: tuple_config.clone(),
                checkpoint,
                metadata,
                parent_config: parent_checkpoint_id.map(|parent_checkpoint_id| {
                    let mut parent = tuple_config.clone();
                    parent.checkpoint_id = Some(parent_checkpoint_id);
                    parent
                }),
                pending_writes,
            });

            remaining = remaining.saturating_sub(1);
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
        let checkpoint_id = checkpoint.id.clone();
        let mut checkpoint = project_checkpoint_for_storage(checkpoint, config);
        for (channel_name, version) in new_versions {
            checkpoint.channel_versions.insert(channel_name, version);
        }

        let metadata = get_serializable_checkpoint_metadata(config, &metadata);
        let checkpoint_json = serde_json::to_string(&checkpoint).map_err(|err| {
            CheckpointError::serialization(format!(
                "failed to serialize checkpoint '{checkpoint_id}': {err}"
            ))
        })?;
        let metadata_json = serde_json::to_string(&metadata).map_err(|err| {
            CheckpointError::serialization(format!(
                "failed to serialize checkpoint metadata '{checkpoint_id}': {err}"
            ))
        })?;

        let mut conn = self.open_connection()?;
        let tx = conn.transaction().map_err(|err| {
            CheckpointError::storage(format!("failed to start put transaction: {err}"))
        })?;

        tx.execute(
            r#"
            INSERT INTO checkpoints (
                thread_id,
                checkpoint_ns,
                checkpoint_id,
                parent_checkpoint_id,
                ts,
                run_id,
                source,
                step,
                checkpoint_json,
                metadata_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(thread_id, checkpoint_ns, checkpoint_id)
            DO UPDATE SET
                parent_checkpoint_id = excluded.parent_checkpoint_id,
                ts = excluded.ts,
                run_id = excluded.run_id,
                source = excluded.source,
                step = excluded.step,
                checkpoint_json = excluded.checkpoint_json,
                metadata_json = excluded.metadata_json
            "#,
            params![
                &thread_id,
                &checkpoint_ns,
                &checkpoint_id,
                parent_checkpoint_id,
                &checkpoint.ts,
                metadata.run_id.clone(),
                metadata
                    .source
                    .map(checkpoint_source_str)
                    .map(str::to_owned),
                metadata.step,
                checkpoint_json,
                metadata_json,
            ],
        )
        .map_err(|err| {
            CheckpointError::storage(format!(
                "failed to persist checkpoint '{checkpoint_id}' in sqlite: {err}"
            ))
        })?;

        tx.commit().map_err(|err| {
            CheckpointError::storage(format!("failed to commit put transaction: {err}"))
        })?;

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

        let mut conn = self.open_connection()?;
        let tx = conn.transaction().map_err(|err| {
            CheckpointError::storage(format!("failed to start put_writes transaction: {err}"))
        })?;

        for (idx, (channel_name, value)) in writes.iter().enumerate() {
            let value_json = serde_json::to_string(value).map_err(|err| {
                CheckpointError::serialization(format!(
                    "failed to serialize write value for task '{task_id}': {err}"
                ))
            })?;
            let write_idx = write_idx_for_channel(channel_name, idx);

            if write_idx >= 0 {
                tx.execute(
                    r#"
                    INSERT OR IGNORE INTO writes (
                        thread_id,
                        checkpoint_ns,
                        checkpoint_id,
                        task_id,
                        write_idx,
                        channel,
                        value_json,
                        task_path
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    "#,
                    params![
                        &config.thread_id,
                        &config.checkpoint_ns,
                        checkpoint_id,
                        task_id,
                        write_idx,
                        channel_name,
                        value_json,
                        task_path
                    ],
                )
                .map_err(|err| {
                    CheckpointError::storage(format!(
                        "failed to insert regular write for task '{task_id}': {err}"
                    ))
                })?;
            } else {
                tx.execute(
                    r#"
                    INSERT INTO writes (
                        thread_id,
                        checkpoint_ns,
                        checkpoint_id,
                        task_id,
                        write_idx,
                        channel,
                        value_json,
                        task_path
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    ON CONFLICT(thread_id, checkpoint_ns, checkpoint_id, task_id, write_idx)
                    DO UPDATE SET
                        channel = excluded.channel,
                        value_json = excluded.value_json,
                        task_path = excluded.task_path
                    "#,
                    params![
                        &config.thread_id,
                        &config.checkpoint_ns,
                        checkpoint_id,
                        task_id,
                        write_idx,
                        channel_name,
                        value_json,
                        task_path
                    ],
                )
                .map_err(|err| {
                    CheckpointError::storage(format!(
                        "failed to upsert special write for task '{task_id}': {err}"
                    ))
                })?;
            }
        }

        tx.commit().map_err(|err| {
            CheckpointError::storage(format!("failed to commit put_writes transaction: {err}"))
        })?;

        Ok(())
    }

    fn delete_thread(&self, thread_id: &str) -> Result<(), CheckpointError> {
        let mut conn = self.open_connection()?;
        let tx = conn.transaction().map_err(|err| {
            CheckpointError::storage(format!("failed to start delete_thread transaction: {err}"))
        })?;

        tx.execute(
            "DELETE FROM writes WHERE thread_id = ?1",
            params![thread_id],
        )
        .map_err(|err| {
            CheckpointError::storage(format!("failed deleting thread writes in sqlite: {err}"))
        })?;
        tx.execute(
            "DELETE FROM checkpoints WHERE thread_id = ?1",
            params![thread_id],
        )
        .map_err(|err| {
            CheckpointError::storage(format!(
                "failed deleting thread checkpoints in sqlite: {err}"
            ))
        })?;

        tx.commit().map_err(|err| {
            CheckpointError::storage(format!("failed to commit delete_thread transaction: {err}"))
        })?;
        Ok(())
    }

    fn delete_for_runs(&self, run_ids: &[String]) -> Result<(), CheckpointError> {
        if run_ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.open_connection()?;
        let tx = conn.transaction().map_err(|err| {
            CheckpointError::storage(format!(
                "failed to start delete_for_runs transaction: {err}"
            ))
        })?;

        for run_id in run_ids {
            tx.execute("DELETE FROM checkpoints WHERE run_id = ?1", params![run_id])
                .map_err(|err| {
                    CheckpointError::storage(format!(
                        "failed deleting checkpoints for run_id '{run_id}': {err}"
                    ))
                })?;
        }

        tx.commit().map_err(|err| {
            CheckpointError::storage(format!(
                "failed to commit delete_for_runs transaction: {err}"
            ))
        })?;

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

        let mut conn = self.open_connection()?;
        let tx = conn.transaction().map_err(|err| {
            CheckpointError::storage(format!("failed to start copy_thread transaction: {err}"))
        })?;

        let source_exists = tx
            .query_row(
                "SELECT 1 FROM checkpoints WHERE thread_id = ?1 LIMIT 1",
                params![source_thread_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|err| {
                CheckpointError::storage(format!(
                    "failed checking source thread '{source_thread_id}' existence: {err}"
                ))
            })?
            .is_some();

        if !source_exists {
            tx.commit().map_err(|err| {
                CheckpointError::storage(format!(
                    "failed to commit copy_thread no-op transaction: {err}"
                ))
            })?;
            return Ok(());
        }

        tx.execute(
            "DELETE FROM writes WHERE thread_id = ?1",
            params![target_thread_id],
        )
        .map_err(|err| {
            CheckpointError::storage(format!(
                "failed deleting target writes for thread '{target_thread_id}': {err}"
            ))
        })?;
        tx.execute(
            "DELETE FROM checkpoints WHERE thread_id = ?1",
            params![target_thread_id],
        )
        .map_err(|err| {
            CheckpointError::storage(format!(
                "failed deleting target checkpoints for thread '{target_thread_id}': {err}"
            ))
        })?;

        tx.execute(
            r#"
            INSERT INTO checkpoints (
                thread_id,
                checkpoint_ns,
                checkpoint_id,
                parent_checkpoint_id,
                ts,
                run_id,
                source,
                step,
                checkpoint_json,
                metadata_json
            )
            SELECT
                ?1,
                checkpoint_ns,
                checkpoint_id,
                parent_checkpoint_id,
                ts,
                run_id,
                source,
                step,
                checkpoint_json,
                metadata_json
            FROM checkpoints
            WHERE thread_id = ?2
            "#,
            params![target_thread_id, source_thread_id],
        )
        .map_err(|err| {
            CheckpointError::storage(format!(
                "failed copying checkpoints from '{source_thread_id}' to '{target_thread_id}': {err}"
            ))
        })?;

        tx.execute(
            r#"
            INSERT INTO writes (
                thread_id,
                checkpoint_ns,
                checkpoint_id,
                task_id,
                write_idx,
                channel,
                value_json,
                task_path
            )
            SELECT
                ?1,
                checkpoint_ns,
                checkpoint_id,
                task_id,
                write_idx,
                channel,
                value_json,
                task_path
            FROM writes
            WHERE thread_id = ?2
            "#,
            params![target_thread_id, source_thread_id],
        )
        .map_err(|err| {
            CheckpointError::storage(format!(
                "failed copying writes from '{source_thread_id}' to '{target_thread_id}': {err}"
            ))
        })?;

        tx.commit().map_err(|err| {
            CheckpointError::storage(format!("failed to commit copy_thread transaction: {err}"))
        })?;
        Ok(())
    }

    fn prune(&self, thread_ids: &[String], strategy: PruneStrategy) -> Result<(), CheckpointError> {
        if thread_ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.open_connection()?;
        let tx = conn.transaction().map_err(|err| {
            CheckpointError::storage(format!("failed to start prune transaction: {err}"))
        })?;

        match strategy {
            PruneStrategy::Delete => {
                for thread_id in thread_ids {
                    tx.execute(
                        "DELETE FROM writes WHERE thread_id = ?1",
                        params![thread_id],
                    )
                    .map_err(|err| {
                        CheckpointError::storage(format!(
                            "failed deleting writes for thread '{thread_id}' during prune: {err}"
                        ))
                    })?;
                    tx.execute("DELETE FROM checkpoints WHERE thread_id = ?1", params![thread_id])
                        .map_err(|err| {
                            CheckpointError::storage(format!(
                                "failed deleting checkpoints for thread '{thread_id}' during prune: {err}"
                            ))
                        })?;
                }
            }
            PruneStrategy::KeepLatest => {
                for thread_id in thread_ids {
                    let mut latest_by_namespace = BTreeMap::<String, String>::new();
                    {
                        let mut stmt = tx
                            .prepare(
                                r#"
                                SELECT checkpoint_ns, MAX(checkpoint_id) AS latest_checkpoint_id
                                FROM checkpoints
                                WHERE thread_id = ?1
                                GROUP BY checkpoint_ns
                                "#,
                            )
                            .map_err(|err| {
                                CheckpointError::storage(format!(
                                    "failed preparing latest checkpoint query for thread '{thread_id}': {err}"
                                ))
                            })?;

                        let mut rows = stmt.query(params![thread_id]).map_err(|err| {
                            CheckpointError::storage(format!(
                                "failed running latest checkpoint query for thread '{thread_id}': {err}"
                            ))
                        })?;

                        while let Some(row) = rows.next().map_err(|err| {
                            CheckpointError::storage(format!(
                                "failed iterating latest checkpoint rows for thread '{thread_id}': {err}"
                            ))
                        })? {
                            let checkpoint_ns: String = row.get(0).map_err(|err| {
                                CheckpointError::storage(format!(
                                    "failed reading checkpoint namespace for thread '{thread_id}': {err}"
                                ))
                            })?;
                            let checkpoint_id: String = row.get(1).map_err(|err| {
                                CheckpointError::storage(format!(
                                    "failed reading latest checkpoint id for thread '{thread_id}': {err}"
                                ))
                            })?;
                            latest_by_namespace.insert(checkpoint_ns, checkpoint_id);
                        }
                    }

                    if latest_by_namespace.is_empty() {
                        continue;
                    }

                    let mut to_delete = Vec::<(String, String)>::new();
                    {
                        let mut stmt = tx
                            .prepare(
                                r#"
                                SELECT checkpoint_ns, checkpoint_id
                                FROM checkpoints
                                WHERE thread_id = ?1
                                "#,
                            )
                            .map_err(|err| {
                                CheckpointError::storage(format!(
                                    "failed preparing prune row query for thread '{thread_id}': {err}"
                                ))
                            })?;

                        let mut rows = stmt.query(params![thread_id]).map_err(|err| {
                            CheckpointError::storage(format!(
                                "failed running prune row query for thread '{thread_id}': {err}"
                            ))
                        })?;

                        while let Some(row) = rows.next().map_err(|err| {
                            CheckpointError::storage(format!(
                                "failed iterating prune row query for thread '{thread_id}': {err}"
                            ))
                        })? {
                            let checkpoint_ns: String = row.get(0).map_err(|err| {
                                CheckpointError::storage(format!(
                                    "failed reading prune namespace for thread '{thread_id}': {err}"
                                ))
                            })?;
                            let checkpoint_id: String = row.get(1).map_err(|err| {
                                CheckpointError::storage(format!(
                                    "failed reading prune checkpoint id for thread '{thread_id}': {err}"
                                ))
                            })?;

                            let keep = latest_by_namespace
                                .get(&checkpoint_ns)
                                .is_some_and(|latest| latest == &checkpoint_id);
                            if !keep {
                                to_delete.push((checkpoint_ns, checkpoint_id));
                            }
                        }
                    }

                    for (checkpoint_ns, checkpoint_id) in to_delete {
                        tx.execute(
                            r#"
                            DELETE FROM checkpoints
                            WHERE thread_id = ?1
                              AND checkpoint_ns = ?2
                              AND checkpoint_id = ?3
                            "#,
                            params![thread_id, checkpoint_ns, checkpoint_id],
                        )
                        .map_err(|err| {
                            CheckpointError::storage(format!(
                                "failed deleting pruned checkpoint for thread '{thread_id}': {err}"
                            ))
                        })?;
                    }
                }
            }
        }

        tx.commit().map_err(|err| {
            CheckpointError::storage(format!("failed to commit prune transaction: {err}"))
        })?;

        Ok(())
    }
}

fn config_matches(
    config: Option<&CheckpointConfig>,
    thread_id: &str,
    checkpoint_ns: &str,
    checkpoint_id: &str,
) -> bool {
    let Some(config) = config else {
        return true;
    };
    if config.thread_id != thread_id {
        return false;
    }
    if config.checkpoint_ns != checkpoint_ns {
        return false;
    }
    if let Some(config_checkpoint_id) = &config.checkpoint_id {
        if config_checkpoint_id != checkpoint_id {
            return false;
        }
    }
    true
}

fn before_filter_blocks(
    before: Option<&CheckpointConfig>,
    thread_id: &str,
    checkpoint_ns: &str,
    checkpoint_id: &str,
) -> bool {
    let Some(before) = before else {
        return false;
    };
    if before.thread_id != thread_id || before.checkpoint_ns != checkpoint_ns {
        return false;
    }
    before
        .checkpoint_id
        .as_ref()
        .is_some_and(|before_checkpoint_id| checkpoint_id >= before_checkpoint_id.as_str())
}

fn metadata_matches(filter: &BTreeMap<String, Value>, metadata: &CheckpointMetadata) -> bool {
    if filter.is_empty() {
        return true;
    }
    filter.iter().all(|(key, expected)| {
        metadata_field_value(metadata, key)
            .as_ref()
            .is_some_and(|actual| actual == expected)
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
    use std::{collections::BTreeMap, env::temp_dir, fs};

    use serde_json::json;
    use uuid::Uuid;

    use crate::langgraph_rs::checkpoint::{
        base::{
            Checkpoint, CheckpointConfig, CheckpointMetadata, CheckpointSaver, CheckpointSource,
            ListCheckpointsQuery, PruneStrategy,
        },
        sqlite::SqliteSaver,
    };

    fn with_saver(test_name: &str, run: impl FnOnce(&SqliteSaver)) {
        let path = temp_dir().join(format!(
            "langgraph_rs_sqlite_{test_name}_{}.db",
            Uuid::new_v4()
        ));
        let saver = SqliteSaver::new(&path).unwrap();
        run(&saver);
        drop(saver);
        let _ = fs::remove_file(path);
    }

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
        with_saver("put_get_latest", |saver| {
            let base = CheckpointConfig::new("thread-1");
            let first = saver
                .put(
                    &base,
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

            let latest = saver.get_tuple(&base).unwrap().unwrap();
            assert_eq!(latest.checkpoint.id, "0002");
            assert_eq!(
                latest
                    .parent_config
                    .as_ref()
                    .and_then(|config| config.checkpoint_id.clone()),
                Some("0001".to_owned())
            );
            assert_eq!(second.checkpoint_id, Some("0002".to_owned()));
        });
    }

    #[test]
    fn put_writes_dedupes_regular_indices_and_overwrites_special_channels() {
        with_saver("put_writes", |saver| {
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
        });
    }

    #[test]
    fn list_applies_before_limit_and_metadata_filter() {
        with_saver("list_filters", |saver| {
            let mut md_a = CheckpointMetadata {
                source: Some(CheckpointSource::Loop),
                step: Some(1),
                parents: BTreeMap::new(),
                run_id: Some("run-a".to_owned()),
                extra: BTreeMap::new(),
            };
            md_a.extra.insert("kind".to_owned(), json!("a"));

            let md_b = CheckpointMetadata {
                source: Some(CheckpointSource::Loop),
                step: Some(2),
                parents: BTreeMap::new(),
                run_id: Some("run-b".to_owned()),
                extra: BTreeMap::new(),
            };

            let c1 = saver
                .put(
                    &CheckpointConfig::new("thread-1"),
                    checkpoint("0001", 1),
                    md_a,
                    BTreeMap::new(),
                )
                .unwrap();
            let c2 = saver
                .put(&c1, checkpoint("0002", 2), md_b, BTreeMap::new())
                .unwrap();
            saver
                .put(
                    &c2,
                    checkpoint("0003", 3),
                    CheckpointMetadata::default(),
                    BTreeMap::new(),
                )
                .unwrap();

            let listed = saver
                .list(&ListCheckpointsQuery {
                    config: Some(CheckpointConfig::new("thread-1")),
                    metadata_filter: BTreeMap::from([("run_id".to_owned(), json!("run-b"))]),
                    before: Some(CheckpointConfig::new("thread-1").with_checkpoint_id("0003")),
                    limit: Some(5),
                })
                .unwrap();

            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].checkpoint.id, "0002");
        });
    }

    #[test]
    fn delete_thread_removes_checkpoints_and_writes() {
        with_saver("delete_thread", |saver| {
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
        });
    }

    #[test]
    fn copy_thread_clones_checkpoints_and_writes() {
        with_saver("copy_thread", |saver| {
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
        });
    }

    #[test]
    fn prune_keep_latest_and_delete_for_runs_work() {
        with_saver("prune_delete_runs", |saver| {
            let keep_md = CheckpointMetadata {
                source: Some(CheckpointSource::Loop),
                step: Some(1),
                parents: BTreeMap::new(),
                run_id: Some("run-keep".to_owned()),
                extra: BTreeMap::new(),
            };

            let delete_md = CheckpointMetadata {
                source: Some(CheckpointSource::Loop),
                step: Some(2),
                parents: BTreeMap::new(),
                run_id: Some("run-delete".to_owned()),
                extra: BTreeMap::new(),
            };

            let c1 = saver
                .put(
                    &CheckpointConfig::new("thread-1"),
                    checkpoint("0001", 1),
                    keep_md,
                    BTreeMap::new(),
                )
                .unwrap();
            saver
                .put(&c1, checkpoint("0002", 2), delete_md, BTreeMap::new())
                .unwrap();

            saver.delete_for_runs(&["run-delete".to_owned()]).unwrap();
            let after_delete = saver
                .list(&ListCheckpointsQuery {
                    config: Some(CheckpointConfig::new("thread-1")),
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(after_delete.len(), 1);
            assert_eq!(after_delete[0].checkpoint.id, "0001");

            let c3 = saver
                .put(
                    &CheckpointConfig::new("thread-1"),
                    checkpoint("0003", 3),
                    CheckpointMetadata::default(),
                    BTreeMap::new(),
                )
                .unwrap();
            saver
                .put(
                    &c3,
                    checkpoint("0004", 4),
                    CheckpointMetadata::default(),
                    BTreeMap::new(),
                )
                .unwrap();

            saver
                .prune(&["thread-1".to_owned()], PruneStrategy::KeepLatest)
                .unwrap();
            let after_prune = saver
                .list(&ListCheckpointsQuery {
                    config: Some(CheckpointConfig::new("thread-1")),
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(after_prune.len(), 1);
            assert_eq!(after_prune[0].checkpoint.id, "0004");
        });
    }
}
