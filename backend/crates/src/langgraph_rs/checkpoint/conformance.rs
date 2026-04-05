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
        memory::InMemorySaver,
        postgres::PostgresSaver,
        sqlite::SqliteSaver,
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

    fn thread(scope: &str, case_name: &str) -> String {
        format!("{scope}-{case_name}")
    }

    fn run_conformance_suite<S: CheckpointSaver>(saver: &S, scope: &str) {
        case_put_get_latest(saver, scope);
        case_put_writes(saver, scope);
        case_list_filters(saver, scope);
        case_delete_thread(saver, scope);
        case_copy_thread(saver, scope);
        case_prune_and_delete_for_runs(saver, scope);
    }

    fn case_put_get_latest<S: CheckpointSaver>(saver: &S, scope: &str) {
        let thread_id = thread(scope, "put_get_latest");
        let _ = saver.delete_thread(&thread_id);

        let base = CheckpointConfig::new(thread_id.clone());
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

        saver.delete_thread(&thread_id).unwrap();
    }

    fn case_put_writes<S: CheckpointSaver>(saver: &S, scope: &str) {
        let thread_id = thread(scope, "put_writes");
        let _ = saver.delete_thread(&thread_id);

        let config = saver
            .put(
                &CheckpointConfig::new(thread_id.clone()),
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

        saver.delete_thread(&thread_id).unwrap();
    }

    fn case_list_filters<S: CheckpointSaver>(saver: &S, scope: &str) {
        let thread_id = thread(scope, "list_filters");
        let _ = saver.delete_thread(&thread_id);

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
                &CheckpointConfig::new(thread_id.clone()),
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
                config: Some(CheckpointConfig::new(thread_id.clone())),
                metadata_filter: BTreeMap::from([("run_id".to_owned(), json!("run-b"))]),
                before: Some(CheckpointConfig::new(thread_id.clone()).with_checkpoint_id("0003")),
                limit: Some(5),
            })
            .unwrap();

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].checkpoint.id, "0002");

        saver.delete_thread(&thread_id).unwrap();
    }

    fn case_delete_thread<S: CheckpointSaver>(saver: &S, scope: &str) {
        let thread_id = thread(scope, "delete_thread");
        let _ = saver.delete_thread(&thread_id);

        let config = saver
            .put(
                &CheckpointConfig::new(thread_id.clone()),
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

        saver.delete_thread(&thread_id).unwrap();
        assert!(
            saver
                .get_tuple(&CheckpointConfig::new(thread_id.clone()))
                .unwrap()
                .is_none()
        );
        assert!(
            saver
                .list(&ListCheckpointsQuery {
                    config: Some(CheckpointConfig::new(thread_id.clone())),
                    ..Default::default()
                })
                .unwrap()
                .is_empty()
        );
    }

    fn case_copy_thread<S: CheckpointSaver>(saver: &S, scope: &str) {
        let source_thread = thread(scope, "copy_source");
        let target_thread = thread(scope, "copy_target");
        let _ = saver.delete_thread(&source_thread);
        let _ = saver.delete_thread(&target_thread);

        let source_config = saver
            .put(
                &CheckpointConfig::new(source_thread.clone()),
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

        saver.copy_thread(&source_thread, &target_thread).unwrap();

        let target_tuple = saver
            .get_tuple(&CheckpointConfig::new(target_thread.clone()))
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

        saver.delete_thread(&source_thread).unwrap();
        saver.delete_thread(&target_thread).unwrap();
    }

    fn case_prune_and_delete_for_runs<S: CheckpointSaver>(saver: &S, scope: &str) {
        let thread_id = thread(scope, "prune_and_delete_runs");
        let _ = saver.delete_thread(&thread_id);

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
                &CheckpointConfig::new(thread_id.clone()),
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
                config: Some(CheckpointConfig::new(thread_id.clone())),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(after_delete.len(), 1);
        assert_eq!(after_delete[0].checkpoint.id, "0001");

        let c3 = saver
            .put(
                &CheckpointConfig::new(thread_id.clone()),
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

        let ns_config = CheckpointConfig::new(thread_id.clone()).with_namespace("chat");
        let c5 = saver
            .put(
                &ns_config,
                checkpoint("1001", 10),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();
        saver
            .put(
                &c5,
                checkpoint("1002", 20),
                CheckpointMetadata::default(),
                BTreeMap::new(),
            )
            .unwrap();

        saver
            .prune(std::slice::from_ref(&thread_id), PruneStrategy::KeepLatest)
            .unwrap();

        let default_ns = saver
            .list(&ListCheckpointsQuery {
                config: Some(CheckpointConfig::new(thread_id.clone())),
                ..Default::default()
            })
            .unwrap();
        let chat_ns = saver
            .list(&ListCheckpointsQuery {
                config: Some(CheckpointConfig::new(thread_id.clone()).with_namespace("chat")),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(default_ns.len(), 1);
        assert_eq!(default_ns[0].checkpoint.id, "0004");
        assert_eq!(chat_ns.len(), 1);
        assert_eq!(chat_ns[0].checkpoint.id, "1002");

        saver.delete_thread(&thread_id).unwrap();
    }

    #[test]
    fn memory_backend_conformance_suite() {
        let saver = InMemorySaver::new();
        run_conformance_suite(&saver, "memory");
    }

    #[test]
    fn sqlite_backend_conformance_suite() {
        let path = temp_dir().join(format!(
            "langgraph_rs_sqlite_conformance_{}.db",
            Uuid::new_v4()
        ));
        let saver = SqliteSaver::new(&path).unwrap();
        run_conformance_suite(&saver, "sqlite");
        drop(saver);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn postgres_backend_conformance_suite() {
        let Ok(connection_string) = std::env::var("LANGGRAPH_RS_TEST_POSTGRES_URL") else {
            return;
        };
        let scope = format!("postgres-{}", Uuid::new_v4().simple());
        let saver = PostgresSaver::new(connection_string).unwrap();
        run_conformance_suite(&saver, &scope);
    }
}
