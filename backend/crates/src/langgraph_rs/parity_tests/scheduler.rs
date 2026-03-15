#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use serde_json::json;

    use crate::langgraph_rs::core::{
        channels::{Channel, LastValue, Topic},
        constants::TASKS,
        scheduler::{
            NodeScheduleSpec, PlannedTaskKind, SchedulerCheckpoint, TaskWrites, apply_writes,
            build_trigger_to_nodes, plan_next_tasks_detailed,
        },
        types::{ChannelWrite, SendPacket},
    };

    #[test]
    fn scheduler_parity_plans_push_and_applies_writes_in_deterministic_order() {
        let mut channels = BTreeMap::<String, Box<dyn Channel>>::new();

        let mut input = LastValue::new("input");
        input.update(&[json!("go")]).unwrap();
        channels.insert("input".to_owned(), Box::new(input));

        let mut tasks = Topic::new(TASKS, false);
        tasks
            .update(&[serde_json::to_value(SendPacket::new("worker", json!({"i": 1}))).unwrap()])
            .unwrap();
        channels.insert(TASKS.to_owned(), Box::new(tasks));

        channels.insert("output".to_owned(), Box::new(Topic::new("output", false)));

        let specs = vec![
            NodeScheduleSpec::new("producer", vec!["input".to_owned()]),
            NodeScheduleSpec::new("worker", Vec::<String>::new()),
        ];
        let trigger_to_nodes = build_trigger_to_nodes(&specs);

        let mut checkpoint = SchedulerCheckpoint::new();
        checkpoint.channel_versions.insert("input".to_owned(), 1);
        checkpoint.channel_versions.insert(TASKS.to_owned(), 1);
        checkpoint.versions_seen.insert(
            "producer".to_owned(),
            BTreeMap::from([("input".to_owned(), 0)]),
        );

        let planned = plan_next_tasks_detailed(
            &checkpoint,
            &channels,
            &specs,
            Some(&trigger_to_nodes),
            Some(&BTreeSet::from(["input".to_owned()])),
            3,
            Some(TASKS),
        )
        .unwrap();

        assert_eq!(planned.len(), 2);
        assert!(matches!(planned[0].kind, PlannedTaskKind::PushSend { .. }));
        assert!(matches!(planned[1].kind, PlannedTaskKind::Pull));

        let push = planned
            .iter()
            .find(|task| matches!(task.kind, PlannedTaskKind::PushSend { .. }))
            .unwrap();
        let pull = planned
            .iter()
            .find(|task| matches!(task.kind, PlannedTaskKind::Pull))
            .unwrap();

        let task_writes = vec![
            TaskWrites::new(
                push.descriptor.clone(),
                push.triggers.clone(),
                vec![ChannelWrite::new("output", json!("from_push"))],
            ),
            TaskWrites::new(
                pull.descriptor.clone(),
                pull.triggers.clone(),
                vec![ChannelWrite::new("output", json!("from_pull"))],
            ),
        ];

        let summary = apply_writes(
            &mut checkpoint,
            &mut channels,
            &task_writes,
            &trigger_to_nodes,
        )
        .unwrap();

        assert_eq!(
            channels.get("output").unwrap().get().unwrap(),
            json!(["from_pull", "from_push"])
        );
        assert_eq!(
            checkpoint
                .versions_seen
                .get("producer")
                .and_then(|seen| seen.get("input"))
                .copied(),
            Some(1)
        );
        assert!(summary.updated_channels.contains("output"));
    }
}
