use std::collections::BTreeSet;

use crate::langgraph_rs::core::types::{ChannelName, TaskDescriptor};

use super::InterruptSelector;

pub fn has_updates_since_last_interrupt(
    channel_versions: &std::collections::BTreeMap<ChannelName, u64>,
    versions_seen_interrupt: Option<&std::collections::BTreeMap<ChannelName, u64>>,
) -> bool {
    channel_versions.iter().any(|(channel, version)| {
        let seen = versions_seen_interrupt
            .and_then(|seen| seen.get(channel))
            .copied()
            .unwrap_or_default();
        *version > seen
    })
}

pub fn should_interrupt(
    channel_versions: &std::collections::BTreeMap<ChannelName, u64>,
    versions_seen_interrupt: Option<&std::collections::BTreeMap<ChannelName, u64>>,
    tasks: &[TaskDescriptor],
    selector: &InterruptSelector,
) -> bool {
    if !selector.is_enabled() {
        return false;
    }
    if !has_updates_since_last_interrupt(channel_versions, versions_seen_interrupt) {
        return false;
    }
    tasks.iter().any(|task| selector.matches_task(task))
}

pub fn interrupted_nodes(tasks: &[TaskDescriptor], selector: &InterruptSelector) -> Vec<String> {
    if !selector.is_enabled() {
        return Vec::new();
    }

    let mut nodes = BTreeSet::<String>::new();
    for task in tasks {
        if selector.matches_task(task) {
            nodes.insert(task.name.clone());
        }
    }

    nodes.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::langgraph_rs::core::types::{TaskDescriptor, TaskPathPart};

    use super::{has_updates_since_last_interrupt, interrupted_nodes, should_interrupt};
    use crate::langgraph_rs::runtime::interrupts::InterruptSelector;

    fn task(id: &str, node: &str) -> TaskDescriptor {
        TaskDescriptor::new(id, node, vec![TaskPathPart::Name("pull".to_owned())])
    }

    #[test]
    fn should_interrupt_when_any_task_matches() {
        let tasks = vec![task("t1", "a"), task("t2", "b")];
        let channel_versions = BTreeMap::from([("a".to_owned(), 2_u64)]);
        let seen = BTreeMap::from([("a".to_owned(), 1_u64)]);

        assert!(should_interrupt(
            &channel_versions,
            Some(&seen),
            &tasks,
            &InterruptSelector::nodes(vec!["b".to_owned()])
        ));
        assert!(!should_interrupt(
            &channel_versions,
            Some(&seen),
            &tasks,
            &InterruptSelector::nodes(vec!["c".to_owned()])
        ));
    }

    #[test]
    fn does_not_interrupt_without_channel_updates_since_last_interrupt() {
        let tasks = vec![task("t1", "a")];
        let channel_versions = BTreeMap::from([("a".to_owned(), 1_u64)]);
        let seen = BTreeMap::from([("a".to_owned(), 1_u64)]);

        assert!(!has_updates_since_last_interrupt(
            &channel_versions,
            Some(&seen)
        ));
        assert!(!should_interrupt(
            &channel_versions,
            Some(&seen),
            &tasks,
            &InterruptSelector::all()
        ));
    }

    #[test]
    fn returns_sorted_unique_interrupted_nodes() {
        let tasks = vec![task("t1", "z"), task("t2", "a"), task("t3", "z")];
        let nodes = interrupted_nodes(&tasks, &InterruptSelector::all());

        assert_eq!(nodes, vec!["a".to_owned(), "z".to_owned()]);
    }
}
