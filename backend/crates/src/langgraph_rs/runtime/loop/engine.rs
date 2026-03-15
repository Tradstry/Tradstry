use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::langgraph_rs::{
    cache::base::Cache,
    checkpoint::base::{
        Checkpoint, CheckpointConfig, CheckpointError, CheckpointMetadata, CheckpointSaver,
        CheckpointSource, PendingWrite, empty_checkpoint_with_config,
    },
    core::{
        channels::{Channel, Topic},
        constants::{ERROR, INPUT, INTERRUPT, NO_WRITES, NULL_TASK_ID, PUSH, RESUME, RETURN},
        managed::{ManagedValueContext, ManagedValueRegistry},
        scheduler::{
            NodeScheduleSpec, PlannedTask, PlannedTaskKind, SchedulerCheckpoint, TaskWrites,
            TriggerToNodes, apply_writes, build_trigger_to_nodes, plan_next_tasks_detailed,
        },
        types::{
            ChannelName, ChannelWrite, CommandGraph, NodeExecutionErrorKind, SendPacket,
            StreamEvent, StreamMode, TaskDescriptor, TaskPathPart, TaskPathStr,
        },
    },
    runtime::{
        interrupts::{interrupted_nodes, should_interrupt},
        io::{
            DebugPayloadType, OutputChannels, OutputWriteGate, TaskOutputWrites,
            map_checkpoint_payload, map_command, map_debug_wrapper, map_input_writes,
            map_output_updates, map_output_values, map_task_payload, map_task_result_payload,
        },
        runner::{RunnerConfig, RunnerError, TaskExecutionRequest, TaskRunner, TaskRuntimeState},
        streaming::{RuntimeEvent, RuntimeStream},
    },
};

use super::{
    DurabilityMode, LoopConfig, LoopError, LoopInput, LoopNodeRunner, LoopRunSummary, LoopStatus,
    LoopTaskReport, StreamParityMode,
    persistence::{AsyncPersistenceWorker, PersistPayload, PersistTaskWrites, persist_payload},
};

#[derive(Debug, Clone)]
pub struct LoopEngine {
    channel_specs: BTreeMap<ChannelName, Box<dyn Channel>>,
    node_specs: Vec<NodeScheduleSpec>,
    trigger_to_nodes: TriggerToNodes,
}

impl LoopEngine {
    pub fn new(
        channel_specs: BTreeMap<ChannelName, Box<dyn Channel>>,
        node_specs: Vec<NodeScheduleSpec>,
    ) -> Self {
        let trigger_to_nodes = build_trigger_to_nodes(&node_specs);
        Self {
            channel_specs,
            node_specs,
            trigger_to_nodes,
        }
    }

    pub fn run(
        &self,
        runner: &dyn LoopNodeRunner,
        saver: Option<&dyn CheckpointSaver>,
        config: LoopConfig,
        input_writes: Vec<ChannelWrite>,
    ) -> Result<LoopRunSummary, LoopError> {
        self.run_with_input(runner, saver, config, LoopInput::Writes(input_writes))
    }

    pub fn run_with_input(
        &self,
        runner: &dyn LoopNodeRunner,
        saver: Option<&dyn CheckpointSaver>,
        config: LoopConfig,
        input: LoopInput,
    ) -> Result<LoopRunSummary, LoopError> {
        self.run_with_stream_and_input_and_cache(runner, saver, config, input, None, None)
    }

    pub fn run_with_stream(
        &self,
        runner: &dyn LoopNodeRunner,
        saver: Option<&dyn CheckpointSaver>,
        config: LoopConfig,
        input_writes: Vec<ChannelWrite>,
        stream: Option<&dyn RuntimeStream>,
    ) -> Result<LoopRunSummary, LoopError> {
        self.run_with_stream_and_input(
            runner,
            saver,
            config,
            LoopInput::Writes(input_writes),
            stream,
        )
    }

    pub fn run_with_stream_and_input(
        &self,
        runner: &dyn LoopNodeRunner,
        saver: Option<&dyn CheckpointSaver>,
        config: LoopConfig,
        input: LoopInput,
        stream: Option<&dyn RuntimeStream>,
    ) -> Result<LoopRunSummary, LoopError> {
        self.run_with_stream_and_input_and_cache(runner, saver, config, input, stream, None)
    }

    pub fn run_with_stream_and_cache(
        &self,
        runner: &dyn LoopNodeRunner,
        saver: Option<&dyn CheckpointSaver>,
        config: LoopConfig,
        input_writes: Vec<ChannelWrite>,
        stream: Option<&dyn RuntimeStream>,
        cache: Option<&dyn Cache>,
    ) -> Result<LoopRunSummary, LoopError> {
        self.run_with_stream_and_input_and_cache(
            runner,
            saver,
            config,
            LoopInput::Writes(input_writes),
            stream,
            cache,
        )
    }

    pub fn run_with_stream_and_input_and_cache(
        &self,
        runner: &dyn LoopNodeRunner,
        saver: Option<&dyn CheckpointSaver>,
        config: LoopConfig,
        input: LoopInput,
        stream: Option<&dyn RuntimeStream>,
        cache: Option<&dyn Cache>,
    ) -> Result<LoopRunSummary, LoopError> {
        std::thread::scope(|scope| {
            self.run_with_stream_and_input_and_cache_scoped(
                scope, runner, saver, config, input, stream, cache,
            )
        })
    }

    fn run_with_stream_and_input_and_cache_scoped<'scope, 'env>(
        &self,
        scope: &'scope std::thread::Scope<'scope, 'env>,
        runner: &dyn LoopNodeRunner,
        saver: Option<&'env dyn CheckpointSaver>,
        config: LoopConfig,
        input: LoopInput,
        stream: Option<&dyn RuntimeStream>,
        cache: Option<&dyn Cache>,
    ) -> Result<LoopRunSummary, LoopError> {
        let (mut checkpoint_config, mut checkpoint, mut metadata, mut pending_writes) =
            load_checkpoint(saver, &config.checkpoint_config)?;

        let effective_channel_specs =
            effective_channel_specs(&self.channel_specs, &config.tasks_channel);
        let output_channels = resolve_output_channels(&config, &effective_channel_specs);
        let mut channels = restore_channels(&effective_channel_specs, &checkpoint)?;
        let untracked_channel_names = collect_untracked_channel_names(&effective_channel_specs);
        let mut scheduler_checkpoint = SchedulerCheckpoint {
            channel_versions: checkpoint.channel_versions.clone(),
            versions_seen: checkpoint.versions_seen.clone(),
            updated_channels: checkpoint
                .updated_channels
                .clone()
                .unwrap_or_default()
                .into_iter()
                .collect(),
        };
        let spec_by_name: BTreeMap<String, &NodeScheduleSpec> = self
            .node_specs
            .iter()
            .map(|spec| (spec.name.clone(), spec))
            .collect();
        let task_runner = TaskRunner::new(
            runner,
            RunnerConfig::new()
                .with_retry_limit(config.retry_limit)
                .with_retry_policies(config.retry_policies.clone())
                .with_max_concurrency(config.max_concurrency),
        )
        .with_stream(stream)
        .with_cache(cache);

        let mut step = metadata.step.unwrap_or(-1).max(-1) + 1;
        let mut steps_executed: u64 = 0;
        let mut tasks_executed: usize = 0;
        let mut reports = Vec::<LoopTaskReport>::new();
        let mut async_persist_worker = match (saver, config.durability) {
            (Some(saver), DurabilityMode::Async) => {
                Some(AsyncPersistenceWorker::spawn(scope, saver))
            }
            _ => None,
        };
        let mut exit_persist_payload = None::<PersistPayload>;
        let mut exit_persist_writes = Vec::<PersistTaskWrites>::new();

        let input_is_none = matches!(&input, LoopInput::None);
        let is_resuming = !checkpoint.channel_versions.is_empty()
            && matches!(&input, LoopInput::None | LoopInput::Command(_));
        if is_resuming {
            let seen_interrupt = scheduler_checkpoint
                .versions_seen
                .entry(INTERRUPT.to_owned())
                .or_default();
            for (channel, version) in &scheduler_checkpoint.channel_versions {
                seen_interrupt.insert(channel.clone(), *version);
            }
            checkpoint.versions_seen = scheduler_checkpoint.versions_seen.clone();
        }

        match input {
            LoopInput::Writes(writes) => {
                for write in map_input_writes(writes, &effective_channel_specs) {
                    pending_writes.push(PendingWrite::new(
                        NULL_TASK_ID,
                        write.channel,
                        write.value,
                    ));
                }
            }
            LoopInput::Command(command) => {
                if command.graph == CommandGraph::Parent {
                    return Err(LoopError::InvalidCommandGraph);
                }
                if command.resume.is_some() && saver.is_none() {
                    return Err(LoopError::InvalidResumeUsage {
                        message: "cannot use Command(resume=...) without checkpointer".to_owned(),
                    });
                }
                let mapped = map_command(&command).map_err(|_| LoopError::InvalidCommandGraph)?;
                if mapped.is_empty() && !is_resuming {
                    return Err(LoopError::EmptyInput);
                }
                for (task_id, channel, value) in mapped {
                    pending_writes.push(PendingWrite::new(task_id, channel, value));
                }
            }
            LoopInput::None => {}
        }

        let null_writes = pending_writes
            .iter()
            .filter(|write| write.task_id == NULL_TASK_ID)
            .map(|write| ChannelWrite::new(write.channel.clone(), write.value.clone()))
            .collect::<Vec<_>>();

        if null_writes.is_empty() && input_is_none && !is_resuming {
            return Err(LoopError::EmptyInput);
        }

        if !null_writes.is_empty() {
            let input_write_count = null_writes.len();
            let resume_count = null_writes
                .iter()
                .filter(|write| write.channel == RESUME)
                .count();
            let input_task = TaskWrites::new(
                TaskDescriptor::new(
                    format!("input:{step}"),
                    INPUT,
                    vec![TaskPathPart::Name("input".to_owned())],
                ),
                Vec::<String>::new(),
                null_writes,
            );

            if resume_count > 0 {
                emit(
                    stream,
                    RuntimeEvent::ResumeApplied {
                        step: step as u64,
                        resumes: resume_count,
                    },
                );
            }
            let summary = apply_writes(
                &mut scheduler_checkpoint,
                &mut channels,
                &[input_task],
                &self.trigger_to_nodes,
            )?;

            sync_checkpoint_from_scheduler(
                &mut checkpoint,
                &scheduler_checkpoint,
                &summary.updated_channels,
            );
            emit(
                stream,
                RuntimeEvent::InputApplied {
                    step: step as u64,
                    writes: input_write_count,
                    updated_channels: summary.updated_channels.iter().cloned().collect(),
                },
            );

            checkpoint = crate::langgraph_rs::checkpoint::base::create_checkpoint_with_config(
                &checkpoint,
                Some(&channels),
                step,
                None,
                Some(&checkpoint_config),
            )?;

            let mut input_metadata = CheckpointMetadata::default();
            input_metadata.source = Some(CheckpointSource::Input);
            input_metadata.step = Some(step);
            input_metadata.parents = parent_map(&checkpoint_config);

            if parity_mode_enabled(&config, StreamMode::Checkpoints)
                || parity_mode_enabled(&config, StreamMode::Debug)
            {
                let values_payload =
                    map_output_values(&output_channels, OutputWriteGate::All, &channels)
                        .unwrap_or(Value::Object(Map::new()));
                let parent_config =
                    build_parent_checkpoint_config(&checkpoint_config, &input_metadata);
                let payload = map_checkpoint_payload(
                    &checkpoint_config,
                    parent_config.as_ref(),
                    &input_metadata,
                    values_payload,
                    &[],
                    &[],
                    &output_channels,
                );
                emit_parity_chunk(
                    stream,
                    &config,
                    StreamMode::Checkpoints,
                    payload,
                    step as u64,
                    Some(DebugPayloadType::Checkpoint),
                );
            }

            if let Some(saver) = saver {
                let payload = PersistPayload {
                    checkpoint: checkpoint.clone(),
                    metadata: input_metadata,
                    new_versions: checkpoint.channel_versions.clone(),
                    writes: Vec::new(),
                    step: step as u64,
                    source: "input",
                };
                match config.durability {
                    DurabilityMode::Sync => {
                        persist_payload_and_emit(saver, &mut checkpoint_config, payload, stream)?
                    }
                    DurabilityMode::Async => {
                        enqueue_async_payload(
                            async_persist_worker.as_mut(),
                            &mut checkpoint_config,
                            payload,
                        )?;
                        drain_async_persistence(
                            async_persist_worker.as_mut(),
                            &mut checkpoint_config,
                            stream,
                        )?;
                    }
                    DurabilityMode::Exit => exit_persist_payload = Some(payload),
                }
            }

            pending_writes.retain(|write| write.task_id != NULL_TASK_ID);
            step += 1;
        }

        if is_resuming && parity_mode_enabled(&config, StreamMode::Values) {
            if let Some(payload) =
                map_output_values(&output_channels, OutputWriteGate::All, &channels)
            {
                emit_mode(stream, StreamMode::Values, payload);
            }
        }

        let mut previous_versions = checkpoint.channel_versions.clone();
        let mut updated_channels = if scheduler_checkpoint.updated_channels.is_empty() {
            None
        } else {
            Some(scheduler_checkpoint.updated_channels.clone())
        };

        let status = loop {
            if steps_executed >= config.recursion_limit {
                break LoopStatus::OutOfSteps;
            }

            let planned = plan_next_tasks_detailed(
                &scheduler_checkpoint,
                &channels,
                &self.node_specs,
                Some(&self.trigger_to_nodes),
                updated_channels.as_ref(),
                step as u64,
                Some(&config.tasks_channel),
            )?;
            if planned.is_empty() {
                break LoopStatus::Done;
            }

            let planned_task_ids = planned
                .iter()
                .map(|task| task.descriptor.id.clone())
                .collect::<Vec<_>>();
            let planned_descriptors = planned
                .iter()
                .map(|task| task.descriptor.clone())
                .collect::<Vec<_>>();
            if should_interrupt(
                &scheduler_checkpoint.channel_versions,
                scheduler_checkpoint.versions_seen.get(INTERRUPT),
                &planned_descriptors,
                &config.interrupt_before,
            ) {
                let interrupt_before_nodes =
                    interrupted_nodes(&planned_descriptors, &config.interrupt_before);
                emit(
                    stream,
                    RuntimeEvent::InterruptBefore {
                        step: step as u64,
                        task_ids: planned_task_ids.clone(),
                        nodes: interrupt_before_nodes,
                    },
                );
                break LoopStatus::InterruptedBefore;
            }

            emit(
                stream,
                RuntimeEvent::StepStarted {
                    step: step as u64,
                    planned_tasks: planned_task_ids.clone(),
                },
            );

            let mut task_writes = Vec::<TaskWrites>::with_capacity(planned.len());
            let mut executed_descriptors = Vec::<TaskDescriptor>::new();
            let mut consumed_pending_ids = BTreeSet::<String>::new();
            let mut scheduled_task_ids = planned_task_ids.iter().cloned().collect::<BTreeSet<_>>();
            let mut task_meta = BTreeMap::<String, (Vec<ChannelName>, PlannedTaskKind)>::new();
            let mut executable_requests = Vec::<TaskExecutionRequest>::new();

            for task in planned {
                let descriptor = task.descriptor;
                let triggers = task.triggers;
                let kind = task.kind;

                let replayed = pending_writes
                    .iter()
                    .filter(|write| write.task_id == descriptor.id)
                    .cloned()
                    .collect::<Vec<_>>();
                if !replayed.is_empty() {
                    let matched_writes = replayed
                        .into_iter()
                        .filter(|write| {
                            write.channel != ERROR
                                && write.channel != INTERRUPT
                                && write.channel != RESUME
                        })
                        .map(|write| ChannelWrite::new(write.channel, write.value))
                        .collect::<Vec<_>>();
                    consumed_pending_ids.insert(descriptor.id.clone());
                    if !matched_writes.is_empty() {
                        reports.push(LoopTaskReport {
                            step: step as u64,
                            task: descriptor.clone(),
                            attempts: 0,
                            writes: matched_writes.clone(),
                        });
                        task_writes.push(TaskWrites::new(
                            descriptor.clone(),
                            triggers.clone(),
                            matched_writes.clone(),
                        ));
                    }
                    if parity_mode_enabled(&config, StreamMode::Updates) {
                        if let Some(payload) = map_output_updates(
                            &output_channels,
                            &[TaskOutputWrites::new(&descriptor, &matched_writes)],
                            true,
                        ) {
                            emit_mode(stream, StreamMode::Updates, payload);
                        }
                    }
                    executed_descriptors.push(descriptor);
                    continue;
                }

                let plan = PlannedTask {
                    descriptor: descriptor.clone(),
                    triggers: triggers.clone(),
                    kind: kind.clone(),
                };
                let request = build_task_request(
                    &plan,
                    &channels,
                    &config.managed_values,
                    &spec_by_name,
                    step as u64,
                    config.recursion_limit,
                )?;
                if parity_mode_enabled(&config, StreamMode::Tasks)
                    || parity_mode_enabled(&config, StreamMode::Debug)
                {
                    let payload = map_task_payload(&descriptor, &request.input, &triggers);
                    emit_parity_chunk(
                        stream,
                        &config,
                        StreamMode::Tasks,
                        payload,
                        step as u64,
                        Some(DebugPayloadType::Task),
                    );
                }
                task_meta.insert(descriptor.id.clone(), (triggers, kind));
                executable_requests.push(request);
            }

            let run_result = task_runner.execute_many_progressive(
                executable_requests,
                config.max_concurrency,
                |executed| {
                    let Some((triggers, _kind)) = task_meta.remove(&executed.task.id) else {
                        return Ok(Vec::new());
                    };

                    let task_for_report = executed.task.clone();
                    let mut output = executed.output;
                    if parity_mode_enabled(&config, StreamMode::Custom) {
                        for event in &output.custom_events {
                            emit_mode(stream, StreamMode::Custom, event.clone());
                        }
                    }
                    if parity_mode_enabled(&config, StreamMode::Messages) {
                        for message_event in &output.message_events {
                            let mut metadata = message_event.metadata.clone();
                            metadata
                                .entry("node".to_owned())
                                .or_insert_with(|| Value::String(task_for_report.name.clone()));
                            metadata
                                .entry("task_id".to_owned())
                                .or_insert_with(|| Value::String(task_for_report.id.clone()));
                            metadata
                                .entry("step".to_owned())
                                .or_insert_with(|| Value::from(step as u64));
                            metadata
                                .entry("attempts".to_owned())
                                .or_insert_with(|| Value::from(executed.attempts as u64));

                            emit_mode(
                                stream,
                                StreamMode::Messages,
                                Value::Array(vec![
                                    message_event.message.clone(),
                                    Value::Object(metadata),
                                ]),
                            );
                        }
                    }

                    let send_writes_from_sends = output
                        .sends
                        .iter()
                        .filter_map(|send| serde_json::to_value(send).ok())
                        .map(|value| ChannelWrite::new(config.tasks_channel.clone(), value))
                        .collect::<Vec<_>>();

                    let send_packets_from_writes =
                        extract_send_packets_from_writes(&output.writes, &config.tasks_channel);
                    let mut send_packets = output.sends.clone();
                    send_packets.extend(send_packets_from_writes.iter().cloned());
                    let send_writes_from_writes = send_packets_from_writes
                        .iter()
                        .filter_map(|send| serde_json::to_value(send).ok())
                        .map(|value| ChannelWrite::new(config.tasks_channel.clone(), value))
                        .collect::<Vec<_>>();

                    let accepted_push = collect_immediate_push_tasks(
                        step as u64,
                        &task_for_report,
                        &send_packets,
                        &spec_by_name,
                        &mut scheduled_task_ids,
                    );
                    if !accepted_push.is_empty() {
                        output.sends.clear();
                        strip_send_writes(&mut output.writes, &config.tasks_channel);
                    }

                    let writes = match TaskWrites::from_execution_result(
                        executed.task,
                        triggers.clone(),
                        output,
                        Some(&config.tasks_channel),
                    ) {
                        Ok(value) => value,
                        Err(err) => {
                            return Err(RunnerError::Execution {
                                task_id: task_for_report.id.clone(),
                                node: task_for_report.name.clone(),
                                attempts: executed.attempts,
                                kind: NodeExecutionErrorKind::Fatal,
                                message: format!(
                                    "failed to convert execution result into task writes: {err}"
                                ),
                            });
                        }
                    };

                    tasks_executed = tasks_executed.saturating_add(1);
                    let was_cached = executed.cached;
                    let mut report_writes = writes.writes.clone();
                    report_writes.extend(send_writes_from_sends);
                    report_writes.extend(send_writes_from_writes);
                    reports.push(LoopTaskReport {
                        step: step as u64,
                        task: task_for_report.clone(),
                        attempts: executed.attempts,
                        writes: report_writes.clone(),
                    });
                    if parity_mode_enabled(&config, StreamMode::Updates) {
                        if let Some(payload) = map_output_updates(
                            &output_channels,
                            &[TaskOutputWrites::new(&task_for_report, &report_writes)],
                            was_cached,
                        ) {
                            emit_mode(stream, StreamMode::Updates, payload);
                        }
                    }
                    if !was_cached
                        && (parity_mode_enabled(&config, StreamMode::Tasks)
                            || parity_mode_enabled(&config, StreamMode::Debug))
                    {
                        let payload = map_task_result_payload(
                            &output_channels,
                            &task_for_report,
                            &report_writes,
                        );
                        emit_parity_chunk(
                            stream,
                            &config,
                            StreamMode::Tasks,
                            payload,
                            step as u64,
                            Some(DebugPayloadType::TaskResult),
                        );
                    }

                    if let (Some(saver), durability) = (saver, config.durability) {
                        if durability == DurabilityMode::Sync {
                            let writes_to_persist = report_writes
                                .iter()
                                .filter_map(|write| {
                                    sanitize_write_for_persistence(
                                        write,
                                        &untracked_channel_names,
                                        &config.tasks_channel,
                                    )
                                })
                                .collect::<Vec<_>>();
                            if !writes_to_persist.is_empty() {
                                let mut write_config = checkpoint_config.clone();
                                write_config.checkpoint_id = Some(checkpoint.id.clone());
                                if let Err(err) = saver.put_writes(
                                    &write_config,
                                    &writes_to_persist,
                                    &task_for_report.id,
                                    &task_for_report.path.to_path_string(),
                                ) {
                                    return Err(RunnerError::Execution {
                                        task_id: task_for_report.id.clone(),
                                        node: task_for_report.name.clone(),
                                        attempts: executed.attempts,
                                        kind: NodeExecutionErrorKind::Fatal,
                                        message: format!("failed to persist task writes: {err}"),
                                    });
                                }
                            }
                        }
                    }

                    task_writes.push(writes);
                    executed_descriptors.push(task_for_report.clone());

                    let mut new_requests = Vec::<TaskExecutionRequest>::new();
                    for push in accepted_push {
                        let push_id = push.descriptor.id.clone();
                        let push_triggers = push.triggers.clone();
                        let push_kind = push.kind.clone();
                        if let Ok(request) = build_task_request(
                            &push,
                            &channels,
                            &config.managed_values,
                            &spec_by_name,
                            step as u64,
                            config.recursion_limit,
                        ) {
                            if parity_mode_enabled(&config, StreamMode::Tasks)
                                || parity_mode_enabled(&config, StreamMode::Debug)
                            {
                                let payload = map_task_payload(
                                    &push.descriptor,
                                    &request.input,
                                    &push.triggers,
                                );
                                emit_parity_chunk(
                                    stream,
                                    &config,
                                    StreamMode::Tasks,
                                    payload,
                                    step as u64,
                                    Some(DebugPayloadType::Task),
                                );
                            }
                            task_meta.insert(push_id, (push_triggers, push_kind));
                            new_requests.push(request);
                        }
                    }
                    Ok(new_requests)
                },
            );
            if let Err(err) = run_result {
                match err {
                    RunnerError::ParentCommand {
                        task_id,
                        node,
                        command,
                    } => {
                        return Err(LoopError::ParentCommand {
                            task_id,
                            node,
                            command,
                        });
                    }
                    other => return Err(LoopError::Runner(other)),
                }
            }

            let summary = apply_writes(
                &mut scheduler_checkpoint,
                &mut channels,
                &task_writes,
                &self.trigger_to_nodes,
            )?;
            updated_channels = Some(summary.updated_channels.clone());
            emit(
                stream,
                RuntimeEvent::WritesApplied {
                    step: step as u64,
                    updated_channels: summary.updated_channels.iter().cloned().collect(),
                },
            );
            if parity_mode_enabled(&config, StreamMode::Values) {
                let step_writes = task_writes
                    .iter()
                    .flat_map(|task| task.writes.iter().cloned())
                    .collect::<Vec<_>>();
                if let Some(payload) = map_output_values(
                    &output_channels,
                    OutputWriteGate::Writes(&step_writes),
                    &channels,
                ) {
                    emit_mode(stream, StreamMode::Values, payload);
                }
            }

            sync_checkpoint_from_scheduler(
                &mut checkpoint,
                &scheduler_checkpoint,
                &summary.updated_channels,
            );
            checkpoint = crate::langgraph_rs::checkpoint::base::create_checkpoint_with_config(
                &checkpoint,
                Some(&channels),
                step,
                None,
                Some(&checkpoint_config),
            )?;

            let new_versions =
                compute_new_versions(&previous_versions, &checkpoint.channel_versions);
            previous_versions = checkpoint.channel_versions.clone();

            pending_writes.retain(|write| !consumed_pending_ids.contains(&write.task_id));

            let mut loop_metadata = CheckpointMetadata::default();
            loop_metadata.source = Some(CheckpointSource::Loop);
            loop_metadata.step = Some(step);
            loop_metadata.parents = parent_map(&checkpoint_config);
            let step_writes = collect_persist_task_writes(
                &reports,
                step as u64,
                &untracked_channel_names,
                &config.tasks_channel,
            );

            if parity_mode_enabled(&config, StreamMode::Checkpoints)
                || parity_mode_enabled(&config, StreamMode::Debug)
            {
                let values_payload =
                    map_output_values(&output_channels, OutputWriteGate::All, &channels)
                        .unwrap_or(Value::Object(Map::new()));
                let task_outputs = task_writes
                    .iter()
                    .map(|task| TaskOutputWrites::new(&task.task, &task.writes))
                    .collect::<Vec<_>>();
                let parent_config =
                    build_parent_checkpoint_config(&checkpoint_config, &loop_metadata);
                let payload = map_checkpoint_payload(
                    &checkpoint_config,
                    parent_config.as_ref(),
                    &loop_metadata,
                    values_payload,
                    &planned_descriptors,
                    &task_outputs,
                    &output_channels,
                );
                emit_parity_chunk(
                    stream,
                    &config,
                    StreamMode::Checkpoints,
                    payload,
                    step as u64,
                    Some(DebugPayloadType::Checkpoint),
                );
            }

            if let Some(saver) = saver {
                let payload = PersistPayload {
                    checkpoint: checkpoint.clone(),
                    metadata: loop_metadata.clone(),
                    new_versions,
                    writes: step_writes.clone(),
                    step: step as u64,
                    source: "loop",
                };
                match config.durability {
                    DurabilityMode::Sync => {
                        persist_payload_and_emit(saver, &mut checkpoint_config, payload, stream)?
                    }
                    DurabilityMode::Async => {
                        enqueue_async_payload(
                            async_persist_worker.as_mut(),
                            &mut checkpoint_config,
                            payload,
                        )?;
                        drain_async_persistence(
                            async_persist_worker.as_mut(),
                            &mut checkpoint_config,
                            stream,
                        )?;
                    }
                    DurabilityMode::Exit => {
                        exit_persist_writes.extend(step_writes);
                        exit_persist_payload = Some(PersistPayload {
                            writes: Vec::new(),
                            ..payload
                        });
                    }
                }
            }

            steps_executed = steps_executed.saturating_add(1);
            if should_interrupt(
                &scheduler_checkpoint.channel_versions,
                scheduler_checkpoint.versions_seen.get(INTERRUPT),
                &executed_descriptors,
                &config.interrupt_after,
            ) {
                let interrupt_after_nodes =
                    interrupted_nodes(&executed_descriptors, &config.interrupt_after);
                emit(
                    stream,
                    RuntimeEvent::InterruptAfter {
                        step: step as u64,
                        task_ids: executed_descriptors
                            .iter()
                            .map(|task| task.id.clone())
                            .collect(),
                        nodes: interrupt_after_nodes,
                    },
                );
                break LoopStatus::InterruptedAfter;
            }
            step += 1;
            metadata.step = Some(step);
        };

        let updated_channels = checkpoint
            .updated_channels
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect::<BTreeSet<_>>();
        if let Some(saver) = saver {
            match config.durability {
                DurabilityMode::Sync => {}
                DurabilityMode::Async => {
                    flush_async_persistence(
                        async_persist_worker.take(),
                        &mut checkpoint_config,
                        stream,
                    )?;
                }
                DurabilityMode::Exit => {
                    let payload = exit_persist_payload.unwrap_or_else(|| PersistPayload {
                        checkpoint: checkpoint.clone(),
                        metadata: metadata.clone(),
                        new_versions: checkpoint.channel_versions.clone(),
                        writes: Vec::new(),
                        step: metadata.step.unwrap_or(step) as u64,
                        source: "loop",
                    });
                    persist_payload_and_emit(
                        saver,
                        &mut checkpoint_config,
                        PersistPayload {
                            writes: exit_persist_writes,
                            ..payload
                        },
                        stream,
                    )?;
                }
            }
        }
        if parity_mode_enabled(&config, StreamMode::Values)
            && matches!(
                status,
                LoopStatus::InterruptedBefore | LoopStatus::InterruptedAfter
            )
        {
            if let Some(payload) =
                map_output_values(&output_channels, OutputWriteGate::All, &channels)
            {
                emit_mode(stream, StreamMode::Values, payload);
            }
        }
        emit(
            stream,
            RuntimeEvent::LoopFinished {
                status: loop_status(status).to_owned(),
                steps_executed,
                tasks_executed,
            },
        );

        Ok(LoopRunSummary {
            status,
            steps_executed,
            tasks_executed,
            final_output: build_final_output(&checkpoint),
            checkpoint,
            checkpoint_config,
            updated_channels,
            task_reports: reports,
        })
    }
}

fn emit(stream: Option<&dyn RuntimeStream>, event: RuntimeEvent) {
    if let Some(stream) = stream {
        stream.emit(event.into_stream_event());
    }
}

fn emit_mode(stream: Option<&dyn RuntimeStream>, mode: StreamMode, payload: Value) {
    if let Some(stream) = stream {
        stream.emit(StreamEvent::new(mode, payload));
    }
}

fn emit_parity_chunk(
    stream: Option<&dyn RuntimeStream>,
    config: &LoopConfig,
    mode: StreamMode,
    payload: Value,
    step: u64,
    debug_type: Option<DebugPayloadType>,
) {
    if parity_mode_enabled(config, mode) {
        emit_mode(stream, mode, payload.clone());
    }
    if let Some(debug_type) = debug_type {
        if parity_mode_enabled(config, StreamMode::Debug) {
            emit_mode(
                stream,
                StreamMode::Debug,
                map_debug_wrapper(step, debug_type, payload),
            );
        }
    }
}

fn parity_mode_enabled(config: &LoopConfig, mode: StreamMode) -> bool {
    if config.stream_parity_mode != StreamParityMode::DualPythonCompat {
        return false;
    }

    match &config.parity_stream_modes {
        Some(modes) => modes.contains(&mode),
        None => matches!(mode, StreamMode::Values | StreamMode::Updates),
    }
}

fn loop_status(status: LoopStatus) -> &'static str {
    match status {
        LoopStatus::Done => "done",
        LoopStatus::OutOfSteps => "out_of_steps",
        LoopStatus::InterruptedBefore => "interrupted_before",
        LoopStatus::InterruptedAfter => "interrupted_after",
    }
}

fn load_checkpoint(
    saver: Option<&dyn CheckpointSaver>,
    config: &CheckpointConfig,
) -> Result<
    (
        CheckpointConfig,
        Checkpoint,
        CheckpointMetadata,
        Vec<PendingWrite>,
    ),
    LoopError,
> {
    if let Some(saver) = saver {
        if let Some(saved) = saver.get_tuple(config)? {
            return Ok((
                saved.config,
                saved.checkpoint,
                saved.metadata,
                saved.pending_writes.unwrap_or_default(),
            ));
        }
    }

    let mut metadata = CheckpointMetadata::default();
    metadata.step = Some(-1);
    Ok((
        config.clone(),
        empty_checkpoint_with_config(Some(config)),
        metadata,
        Vec::new(),
    ))
}

fn restore_channels(
    channel_specs: &BTreeMap<ChannelName, Box<dyn Channel>>,
    checkpoint: &Checkpoint,
) -> Result<BTreeMap<ChannelName, Box<dyn Channel>>, LoopError> {
    let mut channels = BTreeMap::new();
    for (channel_name, channel_spec) in channel_specs {
        let checkpoint_value = checkpoint.channel_values.get(channel_name);
        let mut restored = channel_spec.from_checkpoint(checkpoint_value)?;
        restored.set_key(channel_name.clone());
        channels.insert(channel_name.clone(), restored);
    }
    Ok(channels)
}

fn effective_channel_specs(
    channel_specs: &BTreeMap<ChannelName, Box<dyn Channel>>,
    tasks_channel: &str,
) -> BTreeMap<ChannelName, Box<dyn Channel>> {
    let mut effective = channel_specs.clone();
    effective
        .entry(tasks_channel.to_owned())
        .or_insert_with(|| Box::new(Topic::new(tasks_channel, false)));
    effective
}

fn resolve_output_channels(
    config: &LoopConfig,
    channels: &BTreeMap<ChannelName, Box<dyn Channel>>,
) -> OutputChannels {
    if let Some(output_channels) = &config.output_channels {
        return output_channels.clone();
    }

    let channel_names = channels
        .keys()
        .filter(|channel_name| !is_reserved_output_channel(channel_name, &config.tasks_channel))
        .cloned()
        .collect::<Vec<_>>();
    OutputChannels::Many(channel_names)
}

fn is_reserved_output_channel(channel_name: &str, tasks_channel: &str) -> bool {
    matches!(
        channel_name,
        RESUME | INTERRUPT | ERROR | RETURN | NO_WRITES | PUSH
    ) || channel_name.starts_with("branch:to:")
        || channel_name == tasks_channel
}

fn build_task_input(
    spec: &NodeScheduleSpec,
    channels: &BTreeMap<ChannelName, Box<dyn Channel>>,
    managed: &ManagedValueRegistry,
    step: u64,
    recursion_limit: u64,
) -> Value {
    let mut payload = Map::new();
    let managed_ctx = ManagedValueContext::new(step, recursion_limit);
    for input_channel in spec.effective_input_channels() {
        if let Some(channel) = channels.get(&input_channel) {
            if let Ok(value) = channel.get() {
                payload.insert(input_channel.clone(), value);
            }
            continue;
        }
        if let Some(value) = managed.get(&input_channel) {
            payload.insert(input_channel, value.get(managed_ctx));
        }
    }
    Value::Object(payload)
}

fn build_runtime_state(
    channels: &BTreeMap<ChannelName, Box<dyn Channel>>,
    managed: &ManagedValueRegistry,
    step: u64,
    recursion_limit: u64,
) -> TaskRuntimeState {
    let mut values = BTreeMap::<String, Value>::new();
    for (channel_name, channel) in channels {
        if let Ok(value) = channel.get() {
            values.insert(channel_name.clone(), value);
        }
    }
    let managed_ctx = ManagedValueContext::new(step, recursion_limit);
    for (name, managed_value) in managed {
        values.insert(name.clone(), managed_value.get(managed_ctx));
    }
    TaskRuntimeState::from_values(values)
}

fn build_task_request(
    task: &PlannedTask,
    channels: &BTreeMap<ChannelName, Box<dyn Channel>>,
    managed: &ManagedValueRegistry,
    spec_by_name: &BTreeMap<String, &NodeScheduleSpec>,
    step: u64,
    recursion_limit: u64,
) -> Result<TaskExecutionRequest, LoopError> {
    let spec =
        spec_by_name
            .get(&task.descriptor.name)
            .ok_or_else(|| LoopError::MissingNodeSpec {
                node: task.descriptor.name.clone(),
            })?;

    let input = match &task.kind {
        PlannedTaskKind::Pull => build_task_input(spec, channels, managed, step, recursion_limit),
        PlannedTaskKind::PushSend { packet, .. } => packet.arg.clone(),
    };
    let runtime_state = build_runtime_state(channels, managed, step, recursion_limit);

    Ok(TaskExecutionRequest {
        task: task.descriptor.clone(),
        input,
        step,
        recursion_limit,
        retry_policies: spec.retry_policy.clone(),
        cache_enabled: spec.cache_enabled,
        runtime_state: Some(runtime_state),
    })
}

fn extract_send_packets_from_writes(
    writes: &[ChannelWrite],
    tasks_channel: &str,
) -> Vec<SendPacket> {
    writes
        .iter()
        .filter(|write| write.channel == tasks_channel)
        .filter_map(|write| serde_json::from_value::<SendPacket>(write.value.clone()).ok())
        .collect()
}

fn strip_send_writes(writes: &mut Vec<ChannelWrite>, tasks_channel: &str) {
    writes.retain(|write| {
        if write.channel != tasks_channel {
            return true;
        }
        serde_json::from_value::<SendPacket>(write.value.clone()).is_err()
    });
}

fn collect_immediate_push_tasks(
    step: u64,
    parent_task: &TaskDescriptor,
    sends: &[SendPacket],
    spec_by_name: &BTreeMap<String, &NodeScheduleSpec>,
    scheduled_task_ids: &mut BTreeSet<String>,
) -> Vec<PlannedTask> {
    let mut tasks = Vec::new();
    for (send_idx, send) in sends.iter().enumerate() {
        if !spec_by_name.contains_key(&send.node) {
            continue;
        }
        let task_id = format!("{PUSH}:{step}:{}:{}:{send_idx}", send.node, parent_task.id);
        if !scheduled_task_ids.insert(task_id.clone()) {
            continue;
        }
        let path = vec![
            TaskPathPart::Name("push".to_owned()),
            TaskPathPart::Nested(parent_task.path.clone()),
            TaskPathPart::Index(send_idx as u64),
            TaskPathPart::Name(send.node.clone()),
            TaskPathPart::Index(step),
        ];
        let descriptor = TaskDescriptor::new(task_id, send.node.clone(), path);
        tasks.push(PlannedTask::new_push_send(
            descriptor,
            send_idx,
            send.clone(),
        ));
    }
    tasks
}

fn sync_checkpoint_from_scheduler(
    checkpoint: &mut Checkpoint,
    scheduler_checkpoint: &SchedulerCheckpoint,
    updated_channels: &BTreeSet<ChannelName>,
) {
    checkpoint.channel_versions = scheduler_checkpoint.channel_versions.clone();
    checkpoint.versions_seen = scheduler_checkpoint.versions_seen.clone();
    checkpoint.updated_channels = Some(updated_channels.iter().cloned().collect());
}

fn compute_new_versions(
    previous: &BTreeMap<ChannelName, u64>,
    current: &BTreeMap<ChannelName, u64>,
) -> BTreeMap<ChannelName, u64> {
    current
        .iter()
        .filter_map(|(channel_name, version)| match previous.get(channel_name) {
            Some(previous_version) if previous_version == version => None,
            _ => Some((channel_name.clone(), *version)),
        })
        .collect()
}

fn parent_map(config: &CheckpointConfig) -> BTreeMap<String, String> {
    let mut parents = BTreeMap::new();
    if let Some(checkpoint_id) = &config.checkpoint_id {
        parents.insert(config.checkpoint_ns.clone(), checkpoint_id.clone());
    }
    parents
}

fn build_parent_checkpoint_config(
    config: &CheckpointConfig,
    metadata: &CheckpointMetadata,
) -> Option<CheckpointConfig> {
    metadata
        .parents
        .get(&config.checkpoint_ns)
        .map(|checkpoint_id| {
            let mut parent = config.clone();
            parent.checkpoint_id = Some(checkpoint_id.clone());
            parent
        })
}

fn collect_untracked_channel_names(
    channel_specs: &BTreeMap<ChannelName, Box<dyn Channel>>,
) -> BTreeSet<ChannelName> {
    channel_specs
        .iter()
        .filter_map(|(name, channel)| {
            if channel.kind() == "untracked_value" {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect()
}

fn sanitize_write_for_persistence(
    write: &ChannelWrite,
    untracked_channel_names: &BTreeSet<ChannelName>,
    tasks_channel: &str,
) -> Option<(ChannelName, Value)> {
    if untracked_channel_names.contains(&write.channel) {
        return None;
    }

    if write.channel != tasks_channel || untracked_channel_names.is_empty() {
        return Some((write.channel.clone(), write.value.clone()));
    }

    let mut send = match serde_json::from_value::<SendPacket>(write.value.clone()) {
        Ok(send) => send,
        Err(_) => return Some((write.channel.clone(), write.value.clone())),
    };

    if let Value::Object(arg) = &mut send.arg {
        arg.retain(|channel, _| !untracked_channel_names.contains(channel));
    }

    let sanitized_value = serde_json::to_value(send).unwrap_or_else(|_| write.value.clone());
    Some((write.channel.clone(), sanitized_value))
}

fn collect_persist_task_writes(
    reports: &[LoopTaskReport],
    step: u64,
    untracked_channel_names: &BTreeSet<ChannelName>,
    tasks_channel: &str,
) -> Vec<PersistTaskWrites> {
    reports
        .iter()
        .filter(|report| report.step == step)
        .filter_map(|report| {
            let writes = report
                .writes
                .iter()
                .filter_map(|write| {
                    sanitize_write_for_persistence(write, untracked_channel_names, tasks_channel)
                })
                .collect::<Vec<_>>();
            if writes.is_empty() {
                None
            } else {
                Some(PersistTaskWrites {
                    task_id: report.task.id.clone(),
                    task_path: report.task.path.to_path_string(),
                    writes,
                })
            }
        })
        .collect()
}

fn emit_persist_result(
    stream: Option<&dyn RuntimeStream>,
    step: u64,
    checkpoint_id: String,
    source: &'static str,
) {
    emit(
        stream,
        RuntimeEvent::CheckpointSaved {
            step,
            checkpoint_id,
            source: source.to_owned(),
        },
    );
}

fn persist_payload_and_emit(
    saver: &dyn CheckpointSaver,
    checkpoint_config: &mut CheckpointConfig,
    payload: PersistPayload,
    stream: Option<&dyn RuntimeStream>,
) -> Result<(), LoopError> {
    let result = persist_payload(saver, checkpoint_config, payload)?;
    *checkpoint_config = result.checkpoint_config;
    emit_persist_result(stream, result.step, result.checkpoint_id, result.source);
    Ok(())
}

fn enqueue_async_payload(
    worker: Option<&mut AsyncPersistenceWorker<'_>>,
    checkpoint_config: &mut CheckpointConfig,
    payload: PersistPayload,
) -> Result<(), LoopError> {
    let checkpoint_id = payload.checkpoint.id.clone();
    let worker = worker.ok_or_else(|| {
        LoopError::Checkpoint(CheckpointError::storage(
            "durability mode is async but worker is unavailable",
        ))
    })?;
    worker.enqueue(checkpoint_config.clone(), payload)?;

    // Advance parent linkage optimistically; completions reconcile the canonical config.
    checkpoint_config.checkpoint_id = Some(checkpoint_id);
    Ok(())
}

fn drain_async_persistence(
    worker: Option<&mut AsyncPersistenceWorker<'_>>,
    checkpoint_config: &mut CheckpointConfig,
    stream: Option<&dyn RuntimeStream>,
) -> Result<(), LoopError> {
    if let Some(worker) = worker {
        for result in worker.drain_ready() {
            let result = result?;
            *checkpoint_config = result.checkpoint_config;
            emit_persist_result(stream, result.step, result.checkpoint_id, result.source);
        }
    }
    Ok(())
}

fn flush_async_persistence(
    worker: Option<AsyncPersistenceWorker<'_>>,
    checkpoint_config: &mut CheckpointConfig,
    stream: Option<&dyn RuntimeStream>,
) -> Result<(), LoopError> {
    if let Some(worker) = worker {
        for result in worker.finish() {
            let result = result?;
            *checkpoint_config = result.checkpoint_config;
            emit_persist_result(stream, result.step, result.checkpoint_id, result.source);
        }
    }
    Ok(())
}

fn build_final_output(checkpoint: &Checkpoint) -> Option<Value> {
    if checkpoint.channel_values.is_empty() {
        return None;
    }
    let object = checkpoint
        .channel_values
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Map<_, _>>();
    Some(Value::Object(object))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    };
    use std::time::Duration;

    use serde_json::{Value, json};

    use crate::langgraph_rs::{
        cache::memory::InMemoryCache,
        checkpoint::{base::CheckpointSaver, memory::InMemorySaver},
        core::{
            channels::{Channel, LastValue, Topic, UntrackedValue},
            constants::NULL_TASK_ID,
            scheduler::{NodeScheduleSpec, SchedulerError},
            types::{
                ChannelWrite, Command, CommandGraph, CommandUpdate, ExecutionContext, GotoTarget,
                NodeExecutionError, NodeExecutionResult, SendPacket, StreamMode, TaskPathStr,
            },
        },
        runtime::{
            interrupts::InterruptSelector,
            io::OutputChannels,
            r#loop::{
                DurabilityMode, LoopConfig, LoopEngine, LoopError, LoopInput, LoopNodeRunner,
                LoopStatus, StreamParityMode,
            },
            streaming::StreamCollector,
        },
    };

    struct EchoRunner;

    impl LoopNodeRunner for EchoRunner {
        fn execute(
            &self,
            _node_name: &str,
            input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            let value = input
                .get("input")
                .cloned()
                .ok_or_else(|| NodeExecutionError::fatal("missing input channel"))?;
            Ok(NodeExecutionResult::default().with_write(ChannelWrite::new("output", value)))
        }
    }

    struct StatusOnlyRunner;

    impl LoopNodeRunner for StatusOnlyRunner {
        fn execute(
            &self,
            _node_name: &str,
            _input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            Ok(NodeExecutionResult::default().with_write(ChannelWrite::new("status", json!("ok"))))
        }
    }

    struct TickRunner;

    impl LoopNodeRunner for TickRunner {
        fn execute(
            &self,
            _node_name: &str,
            _input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            Ok(NodeExecutionResult::default().with_write(ChannelWrite::new("tick", json!("next"))))
        }
    }

    struct FailingRunner;

    impl LoopNodeRunner for FailingRunner {
        fn execute(
            &self,
            _node_name: &str,
            _input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            Err(NodeExecutionError::fatal("boom"))
        }
    }

    struct FlakyRunner {
        calls: AtomicU32,
    }

    impl FlakyRunner {
        fn new() -> Self {
            Self {
                calls: AtomicU32::new(0),
            }
        }
    }

    struct UntrackedFlowRunner;

    impl LoopNodeRunner for UntrackedFlowRunner {
        fn execute(
            &self,
            node_name: &str,
            input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            match node_name {
                "producer" => Ok(NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("scratch", json!("secret")))),
                "consumer" => Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                    "output",
                    input
                        .get("scratch")
                        .cloned()
                        .ok_or_else(|| NodeExecutionError::fatal("missing scratch input"))?,
                ))),
                "persist" => Ok(NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("scratch", json!("secret")))
                    .with_write(ChannelWrite::new("output", json!("ok")))
                    .with_send(SendPacket::new(
                        "downstream",
                        json!({"scratch": "secret", "safe": "ok"}),
                    ))),
                other => Err(NodeExecutionError::fatal(format!(
                    "unexpected node '{other}'"
                ))),
            }
        }
    }

    struct PushFlowRunner;

    impl LoopNodeRunner for PushFlowRunner {
        fn execute(
            &self,
            node_name: &str,
            input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            match node_name {
                "producer" => Ok(NodeExecutionResult::default()
                    .with_send(SendPacket::new("worker", json!({"payload": "from_send"})))),
                "worker" => Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                    "output",
                    input
                        .get("payload")
                        .cloned()
                        .ok_or_else(|| NodeExecutionError::fatal("missing payload"))?,
                ))),
                other => Err(NodeExecutionError::fatal(format!(
                    "unexpected node '{other}'"
                ))),
            }
        }
    }

    struct CommandFlowRunner;

    impl LoopNodeRunner for CommandFlowRunner {
        fn execute(
            &self,
            node_name: &str,
            input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            match node_name {
                "echo" => Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                    "output",
                    input
                        .get("input")
                        .cloned()
                        .ok_or_else(|| NodeExecutionError::fatal("missing input channel"))?,
                ))),
                "worker" => Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                    "output",
                    input
                        .get("payload")
                        .cloned()
                        .ok_or_else(|| NodeExecutionError::fatal("missing payload"))?,
                ))),
                other => Err(NodeExecutionError::fatal(format!(
                    "unexpected node '{other}'"
                ))),
            }
        }
    }

    struct StreamingPayloadRunner;

    impl LoopNodeRunner for StreamingPayloadRunner {
        fn execute(
            &self,
            _node_name: &str,
            _input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            Ok(NodeExecutionResult::default()
                .with_write(ChannelWrite::new("output", json!("hello")))
                .with_custom_event(json!({"kind": "custom", "value": 1}))
                .with_message_event_with_metadata(
                    json!("message"),
                    serde_json::Map::from_iter([("source".to_owned(), json!("node"))]),
                ))
        }
    }

    struct ManagedInputRunner;

    impl LoopNodeRunner for ManagedInputRunner {
        fn execute(
            &self,
            _node_name: &str,
            input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            Ok(NodeExecutionResult::default()
                .with_write(ChannelWrite::new(
                    "last_step",
                    input
                        .get("is_last_step")
                        .cloned()
                        .ok_or_else(|| NodeExecutionError::fatal("missing is_last_step"))?,
                ))
                .with_write(ChannelWrite::new(
                    "remaining",
                    input
                        .get("remaining_steps")
                        .cloned()
                        .ok_or_else(|| NodeExecutionError::fatal("missing remaining_steps"))?,
                )))
        }
    }

    struct RuntimeCapabilityRunner;

    impl LoopNodeRunner for RuntimeCapabilityRunner {
        fn execute(
            &self,
            node_name: &str,
            input: Value,
            ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            match node_name {
                "reader" => {
                    let read = ctx
                        .pregel_read(vec!["input".to_owned(), "is_last_step".to_owned()], false)?;
                    let values = read.as_object().cloned().ok_or_else(|| {
                        NodeExecutionError::fatal("runtime read must return an object")
                    })?;
                    ctx.pregel_send(vec![
                        ChannelWrite::new(
                            "output",
                            values.get("input").cloned().unwrap_or(Value::Null),
                        ),
                        ChannelWrite::new(
                            "managed_out",
                            values.get("is_last_step").cloned().unwrap_or(Value::Null),
                        ),
                    ])?;
                    Ok(NodeExecutionResult::default())
                }
                "caller" => {
                    let payload = input.get("input").cloned().unwrap_or(Value::Null);
                    let called = ctx.pregel_call("worker", json!({"payload": payload}))?;
                    ctx.pregel_send(vec![ChannelWrite::new("output", called)])?;
                    Ok(NodeExecutionResult::default())
                }
                "worker" => Ok(NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("called", json!(true)))
                    .with_return_value(
                        input
                            .get("payload")
                            .cloned()
                            .ok_or_else(|| NodeExecutionError::fatal("missing payload"))?,
                    )),
                other => Err(NodeExecutionError::fatal(format!(
                    "unexpected node '{other}'"
                ))),
            }
        }
    }

    #[derive(Default)]
    struct RecordingSaver {
        inner: InMemorySaver,
        operations: Mutex<Vec<String>>,
    }

    impl RecordingSaver {
        fn operations(&self) -> Vec<String> {
            self.operations
                .lock()
                .map(|ops| ops.clone())
                .unwrap_or_default()
        }
    }

    impl CheckpointSaver for RecordingSaver {
        fn get_tuple(
            &self,
            config: &crate::langgraph_rs::checkpoint::base::CheckpointConfig,
        ) -> Result<
            Option<crate::langgraph_rs::checkpoint::base::CheckpointTuple>,
            crate::langgraph_rs::checkpoint::base::CheckpointError,
        > {
            self.inner.get_tuple(config)
        }

        fn put(
            &self,
            config: &crate::langgraph_rs::checkpoint::base::CheckpointConfig,
            checkpoint: crate::langgraph_rs::checkpoint::base::Checkpoint,
            metadata: crate::langgraph_rs::checkpoint::base::CheckpointMetadata,
            new_versions: crate::langgraph_rs::checkpoint::base::ChannelVersions,
        ) -> Result<
            crate::langgraph_rs::checkpoint::base::CheckpointConfig,
            crate::langgraph_rs::checkpoint::base::CheckpointError,
        > {
            std::thread::sleep(Duration::from_millis(15));
            if let Ok(mut ops) = self.operations.lock() {
                ops.push(format!("put:{}", checkpoint.id));
            }
            self.inner.put(config, checkpoint, metadata, new_versions)
        }

        fn put_writes(
            &self,
            config: &crate::langgraph_rs::checkpoint::base::CheckpointConfig,
            writes: &[(String, Value)],
            task_id: &String,
            task_path: &str,
        ) -> Result<(), crate::langgraph_rs::checkpoint::base::CheckpointError> {
            if let Ok(mut ops) = self.operations.lock() {
                if let Some(checkpoint_id) = &config.checkpoint_id {
                    ops.push(format!("writes:{checkpoint_id}"));
                }
            }
            self.inner.put_writes(config, writes, task_id, task_path)
        }
    }

    struct CurrentGraphCommandRunner;

    impl LoopNodeRunner for CurrentGraphCommandRunner {
        fn execute(
            &self,
            _node_name: &str,
            _input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            Ok(
                NodeExecutionResult::default().with_command(Command::new().with_update(
                    CommandUpdate::Object(serde_json::Map::from_iter([(
                        "output".to_owned(),
                        json!("from_node_command"),
                    )])),
                )),
            )
        }
    }

    struct ParentGraphCommandRunner;

    impl LoopNodeRunner for ParentGraphCommandRunner {
        fn execute(
            &self,
            _node_name: &str,
            _input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            Ok(NodeExecutionResult::default().with_command(Command::new().to_parent()))
        }
    }

    impl LoopNodeRunner for FlakyRunner {
        fn execute(
            &self,
            _node_name: &str,
            input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            let attempt = self.calls.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                return Err(NodeExecutionError::retryable("transient"));
            }
            let value = input
                .get("input")
                .cloned()
                .ok_or_else(|| NodeExecutionError::fatal("missing input channel"))?;
            Ok(NodeExecutionResult::default().with_write(ChannelWrite::new("output", value)))
        }
    }

    #[test]
    fn executes_single_superstep_and_persists_checkpoint() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);

        let saver = InMemorySaver::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-loop"),
        )
        .with_recursion_limit(4);

        let result = engine
            .run(
                &EchoRunner,
                Some(&saver),
                config,
                vec![ChannelWrite::new("input", json!("hello"))],
            )
            .unwrap();

        assert_eq!(result.status, LoopStatus::Done);
        assert_eq!(result.steps_executed, 1);
        assert_eq!(result.tasks_executed, 1);
        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("hello"))
        );

        let saved = saver.get_tuple(&result.checkpoint_config).unwrap().unwrap();
        assert_eq!(
            saved.checkpoint.channel_values.get("output"),
            Some(&json!("hello"))
        );
    }

    #[test]
    fn stops_when_recursion_limit_is_reached() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("tick".to_owned(), Box::new(LastValue::new("tick")));

        let node_specs = vec![NodeScheduleSpec::new("ticker", vec!["tick".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-limit"),
        )
        .with_recursion_limit(1);

        let result = engine
            .run(
                &TickRunner,
                None,
                config,
                vec![ChannelWrite::new("tick", json!("start"))],
            )
            .unwrap();

        assert_eq!(result.status, LoopStatus::OutOfSteps);
        assert_eq!(result.steps_executed, 1);
        assert_eq!(result.tasks_executed, 1);
    }

    #[test]
    fn surfaces_task_failures() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));

        let node_specs = vec![NodeScheduleSpec::new("bad", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-fail"),
        );

        let error = engine
            .run(
                &FailingRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("x"))],
            )
            .unwrap_err();

        assert!(format!("{error}").contains("task execution failed"));
    }

    #[test]
    fn delegates_retries_to_runtime_runner_policy() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-retry"),
        )
        .with_retry_limit(1)
        .with_recursion_limit(3);
        let runner = FlakyRunner::new();

        let result = engine
            .run(
                &runner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("hello"))],
            )
            .unwrap();

        assert_eq!(result.tasks_executed, 1);
        assert_eq!(result.task_reports.len(), 1);
        assert_eq!(result.task_reports[0].attempts, 2);
    }

    #[test]
    fn emits_structured_stream_events_per_task_and_superstep() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let saver = InMemorySaver::new();
        let collector = StreamCollector::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-stream"),
        )
        .with_retry_limit(1)
        .with_recursion_limit(3);

        let _result = engine
            .run_with_stream(
                &EchoRunner,
                Some(&saver),
                config,
                vec![ChannelWrite::new("input", json!("hello"))],
                Some(&collector),
            )
            .unwrap();

        let events = collector.events();
        let has = |name: &str| {
            events.iter().any(|event| {
                event.payload.get("event").and_then(|value| value.as_str()) == Some(name)
            })
        };

        assert!(has("input_applied"));
        assert!(has("step_started"));
        assert!(has("task_started"));
        assert!(has("task_succeeded"));
        assert!(has("writes_applied"));
        assert!(has("checkpoint_saved"));
        assert!(has("loop_finished"));
    }

    #[test]
    fn dual_stream_mode_emits_python_compatible_values_and_updates() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let collector = StreamCollector::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-dual-stream"),
        )
        .with_recursion_limit(3)
        .with_stream_parity_mode(StreamParityMode::DualPythonCompat)
        .with_output_channels(OutputChannels::Single("output".to_owned()));

        let _ = engine
            .run_with_stream(
                &EchoRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("hello"))],
                Some(&collector),
            )
            .unwrap();

        let events = collector.events();
        assert!(events.iter().any(|event| {
            event.payload.get("event").and_then(|value| value.as_str()) == Some("task_succeeded")
        }));
        assert!(events.iter().any(|event| {
            event.mode == StreamMode::Updates
                && event.payload.get("echo").is_some()
                && event.payload.get("event").is_none()
        }));
        assert!(events.iter().any(|event| {
            event.mode == StreamMode::Values
                && event.payload == json!("hello")
                && event.payload.get("event").is_none()
        }));
    }

    #[test]
    fn dual_stream_mode_emits_initial_values_on_resume() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let saver = InMemorySaver::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-dual-resume"),
        )
        .with_recursion_limit(3)
        .with_stream_parity_mode(StreamParityMode::DualPythonCompat)
        .with_output_channels(OutputChannels::Single("output".to_owned()));

        let first = engine
            .run(
                &EchoRunner,
                Some(&saver),
                config.clone(),
                vec![ChannelWrite::new("input", json!("hello"))],
            )
            .unwrap();
        assert_eq!(
            first.checkpoint.channel_values.get("output"),
            Some(&json!("hello"))
        );

        let collector = StreamCollector::new();
        let resumed = engine
            .run_with_stream_and_input(
                &EchoRunner,
                Some(&saver),
                config,
                LoopInput::None,
                Some(&collector),
            )
            .unwrap();
        assert_eq!(resumed.status, LoopStatus::Done);

        let events = collector.events();
        assert!(events.iter().any(|event| {
            event.mode == StreamMode::Values
                && event.payload == json!("hello")
                && event.payload.get("event").is_none()
        }));
    }

    #[test]
    fn dual_stream_mode_values_emission_requires_output_channel_write_intersection() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("status".to_owned(), Box::new(LastValue::new("status")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("status", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let collector = StreamCollector::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-dual-gate"),
        )
        .with_recursion_limit(3)
        .with_stream_parity_mode(StreamParityMode::DualPythonCompat)
        .with_output_channels(OutputChannels::Single("output".to_owned()));

        let _ = engine
            .run_with_stream(
                &StatusOnlyRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("hello"))],
                Some(&collector),
            )
            .unwrap();

        let values_events = collector
            .events()
            .into_iter()
            .filter(|event| event.mode == StreamMode::Values)
            .collect::<Vec<_>>();
        assert!(values_events.is_empty());
    }

    #[test]
    fn dual_stream_mode_updates_include_cached_metadata() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let cache = InMemoryCache::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-dual-cache"),
        )
        .with_recursion_limit(3)
        .with_stream_parity_mode(StreamParityMode::DualPythonCompat)
        .with_output_channels(OutputChannels::Single("output".to_owned()));

        let _ = engine
            .run_with_stream_and_cache(
                &EchoRunner,
                None,
                config.clone(),
                vec![ChannelWrite::new("input", json!("hello"))],
                None,
                Some(&cache),
            )
            .unwrap();

        let collector = StreamCollector::new();
        let _ = engine
            .run_with_stream_and_cache(
                &EchoRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("hello"))],
                Some(&collector),
                Some(&cache),
            )
            .unwrap();

        let events = collector.events();
        assert!(events.iter().any(|event| {
            event.mode == StreamMode::Updates
                && event
                    .payload
                    .get("__metadata__")
                    .and_then(|metadata| metadata.get("cached"))
                    .and_then(|value| value.as_bool())
                    == Some(true)
        }));
    }

    #[test]
    fn dual_stream_mode_mode_selection_controls_parity_chunks() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let saver = InMemorySaver::new();
        let collector = StreamCollector::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-parity-modes"),
        )
        .with_stream_parity_mode(StreamParityMode::DualPythonCompat)
        .with_parity_stream_modes(vec![
            StreamMode::Tasks,
            StreamMode::Checkpoints,
            StreamMode::Debug,
        ])
        .with_output_channels(OutputChannels::Single("output".to_owned()))
        .with_recursion_limit(3);

        let _ = engine
            .run_with_stream(
                &EchoRunner,
                Some(&saver),
                config,
                vec![ChannelWrite::new("input", json!("hello"))],
                Some(&collector),
            )
            .unwrap();

        let events = collector.events();
        assert!(events.iter().any(|event| {
            event.mode == StreamMode::Tasks
                && event.payload.get("event").is_none()
                && event.payload.get("id").is_some()
        }));
        assert!(events.iter().any(|event| {
            event.mode == StreamMode::Checkpoints
                && event.payload.get("event").is_none()
                && event.payload.get("config").is_some()
        }));
        assert!(events.iter().any(|event| {
            event.mode == StreamMode::Debug
                && event.payload.get("type").is_some()
                && event.payload.get("payload").is_some()
        }));
        assert!(!events.iter().any(|event| {
            event.mode == StreamMode::Values && event.payload.get("event").is_none()
        }));
        assert!(!events.iter().any(|event| {
            event.mode == StreamMode::Updates
                && event.payload.get("event").is_none()
                && event.payload.get("echo").is_some()
        }));
    }

    #[test]
    fn dual_stream_mode_emits_custom_and_messages_chunks() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("streamer", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let collector = StreamCollector::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-parity-msg"),
        )
        .with_stream_parity_mode(StreamParityMode::DualPythonCompat)
        .with_parity_stream_modes(vec![StreamMode::Custom, StreamMode::Messages])
        .with_output_channels(OutputChannels::Single("output".to_owned()))
        .with_recursion_limit(3);

        let _ = engine
            .run_with_stream(
                &StreamingPayloadRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("go"))],
                Some(&collector),
            )
            .unwrap();

        let events = collector.events();
        assert!(events.iter().any(|event| {
            event.mode == StreamMode::Custom
                && event.payload == json!({"kind": "custom", "value": 1})
        }));
        let message = events
            .iter()
            .find(|event| event.mode == StreamMode::Messages);
        assert!(message.is_some());
        let payload = message.and_then(|event| event.payload.as_array()).cloned();
        assert_eq!(payload.as_ref().map(|values| values.len()), Some(2));
        let metadata = payload
            .as_ref()
            .and_then(|values| values.get(1))
            .and_then(|value| value.as_object())
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            metadata.get("source"),
            Some(&json!("node")),
            "message metadata should keep node-provided keys"
        );
        assert_eq!(metadata.get("node"), Some(&json!("streamer")));
        assert!(metadata.contains_key("task_id"));
        assert!(metadata.contains_key("step"));
        assert!(metadata.contains_key("attempts"));
    }

    #[test]
    fn interrupts_before_task_execution_and_emits_event() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let collector = StreamCollector::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-interrupt-before"),
        )
        .with_recursion_limit(4)
        .with_interrupt_before(InterruptSelector::nodes(vec!["echo".to_owned()]));

        let result = engine
            .run_with_stream(
                &EchoRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("hello"))],
                Some(&collector),
            )
            .unwrap();

        assert_eq!(result.status, LoopStatus::InterruptedBefore);
        assert_eq!(result.steps_executed, 0);
        assert_eq!(result.tasks_executed, 0);
        assert_eq!(result.checkpoint.channel_values.get("output"), None);

        let events = collector.events();
        let has = |name: &str| {
            events.iter().any(|event| {
                event.payload.get("event").and_then(|value| value.as_str()) == Some(name)
            })
        };

        assert!(has("interrupt_before"));
        assert!(!has("task_started"));
        assert!(events.iter().any(|event| {
            event.payload.get("event").and_then(|value| value.as_str()) == Some("loop_finished")
                && event.payload.get("status").and_then(|value| value.as_str())
                    == Some("interrupted_before")
        }));
    }

    #[test]
    fn interrupts_after_step_and_emits_event() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let collector = StreamCollector::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-interrupt-after"),
        )
        .with_recursion_limit(4)
        .with_interrupt_after(InterruptSelector::nodes(vec!["echo".to_owned()]));

        let result = engine
            .run_with_stream(
                &EchoRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("hello"))],
                Some(&collector),
            )
            .unwrap();

        assert_eq!(result.status, LoopStatus::InterruptedAfter);
        assert_eq!(result.steps_executed, 1);
        assert_eq!(result.tasks_executed, 1);
        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("hello"))
        );

        let events = collector.events();
        let has = |name: &str| {
            events.iter().any(|event| {
                event.payload.get("event").and_then(|value| value.as_str()) == Some(name)
            })
        };

        assert!(has("task_started"));
        assert!(has("task_succeeded"));
        assert!(has("interrupt_after"));
        assert!(events.iter().any(|event| {
            event.payload.get("event").and_then(|value| value.as_str()) == Some("loop_finished")
                && event.payload.get("status").and_then(|value| value.as_str())
                    == Some("interrupted_after")
        }));
    }

    #[test]
    fn emits_resume_applied_event_for_resume_input_writes() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let collector = StreamCollector::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-resume"),
        )
        .with_recursion_limit(3);

        let _result = engine
            .run_with_stream(
                &EchoRunner,
                None,
                config,
                vec![
                    ChannelWrite::new(
                        crate::langgraph_rs::checkpoint::base::RESUME_WRITE_CHANNEL,
                        json!({"id": "i1", "value": "continue"}),
                    ),
                    ChannelWrite::new("input", json!("hello")),
                ],
                Some(&collector),
            )
            .unwrap();

        let events = collector.events();
        let resume_event = events
            .iter()
            .find(|event| {
                event.payload.get("event").and_then(|value| value.as_str())
                    == Some("resume_applied")
            })
            .expect("resume_applied event");

        assert_eq!(
            resume_event
                .payload
                .get("resumes")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
    }

    #[test]
    fn command_update_routes_to_state_input_and_executes() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-command-update"),
        )
        .with_recursion_limit(4);

        let command = Command::new().with_update(CommandUpdate::Object(
            serde_json::Map::from_iter([("input".to_owned(), json!("hello"))]),
        ));
        let result = engine
            .run_with_input(
                &CommandFlowRunner,
                None,
                config,
                LoopInput::Command(command),
            )
            .unwrap();

        assert_eq!(result.status, LoopStatus::Done);
        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("hello"))
        );
    }

    #[test]
    fn command_goto_send_executes_push_target() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("worker", Vec::<String>::new())];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-command-goto"),
        )
        .with_recursion_limit(4);

        let command = Command::new().with_goto(GotoTarget::Send(SendPacket::new(
            "worker",
            json!({"payload": "from_command"}),
        )));
        let result = engine
            .run_with_input(
                &CommandFlowRunner,
                None,
                config,
                LoopInput::Command(command),
            )
            .unwrap();

        assert_eq!(result.tasks_executed, 1);
        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("from_command"))
        );
    }

    #[test]
    fn parent_graph_command_is_rejected() {
        let channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        let engine = LoopEngine::new(
            channels,
            vec![NodeScheduleSpec::new("echo", Vec::<String>::new())],
        );
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-parent-cmd"),
        );

        let mut command = Command::new();
        command.graph = CommandGraph::Parent;
        let err = engine
            .run_with_input(
                &CommandFlowRunner,
                None,
                config,
                LoopInput::Command(command),
            )
            .unwrap_err();
        assert!(matches!(err, LoopError::InvalidCommandGraph));
    }

    #[test]
    fn node_current_graph_command_is_applied_as_writes() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("emit", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-node-command"),
        )
        .with_recursion_limit(3);

        let result = engine
            .run_with_input(
                &CurrentGraphCommandRunner,
                None,
                config,
                LoopInput::Writes(vec![ChannelWrite::new("input", json!("go"))]),
            )
            .unwrap();

        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("from_node_command"))
        );
    }

    #[test]
    fn bubbles_parent_command_from_node_execution() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));

        let node_specs = vec![NodeScheduleSpec::new("emit", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-node-parent"),
        )
        .with_recursion_limit(3);

        let err = engine
            .run_with_input(
                &ParentGraphCommandRunner,
                None,
                config,
                LoopInput::Writes(vec![ChannelWrite::new("input", json!("go"))]),
            )
            .unwrap_err();

        assert!(matches!(err, LoopError::ParentCommand { .. }));
    }

    #[test]
    fn replays_null_task_pending_writes_from_checkpoint() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let saver = InMemorySaver::new();

        let base =
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-replay-null");
        let mut metadata = crate::langgraph_rs::checkpoint::base::CheckpointMetadata::default();
        metadata.step = Some(-1);
        let seeded = saver
            .put(
                &base,
                crate::langgraph_rs::checkpoint::base::empty_checkpoint(),
                metadata,
                BTreeMap::new(),
            )
            .unwrap();
        saver
            .put_writes(
                &seeded,
                &[("input".to_owned(), json!("from_pending"))],
                &NULL_TASK_ID.to_owned(),
                "",
            )
            .unwrap();

        let config = LoopConfig::new(base).with_recursion_limit(4);
        let result = engine
            .run_with_input(&CommandFlowRunner, Some(&saver), config, LoopInput::None)
            .unwrap();

        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("from_pending"))
        );
    }

    #[test]
    fn interrupt_gate_requires_updates_since_last_interrupt() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let saver = InMemorySaver::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-interrupt-gate"),
        )
        .with_interrupt_before(InterruptSelector::all())
        .with_recursion_limit(4);

        let first = engine
            .run_with_input(
                &CommandFlowRunner,
                Some(&saver),
                config.clone(),
                LoopInput::Writes(vec![ChannelWrite::new("input", json!("x"))]),
            )
            .unwrap();
        assert_eq!(first.status, LoopStatus::InterruptedBefore);

        let second = engine
            .run_with_input(&CommandFlowRunner, Some(&saver), config, LoopInput::None)
            .unwrap();
        assert_eq!(second.status, LoopStatus::Done);
    }

    #[test]
    fn executes_send_push_tasks_with_send_arg_input() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert(
            "__pregel_tasks".to_owned(),
            Box::new(Topic::new("__pregel_tasks", false)),
        );
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![
            NodeScheduleSpec::new("producer", vec!["input".to_owned()]),
            NodeScheduleSpec::new("worker", Vec::<String>::new()),
        ];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-push"),
        )
        .with_recursion_limit(6);

        let result = engine
            .run(
                &PushFlowRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("start"))],
            )
            .unwrap();

        assert_eq!(result.status, LoopStatus::Done);
        assert_eq!(result.tasks_executed, 2);
        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("from_send"))
        );
        assert!(
            result
                .task_reports
                .iter()
                .any(|report| report.task.id.starts_with("__pregel_push:")),
            "expected at least one push task report"
        );
    }

    #[test]
    fn auto_provisions_tasks_channel_when_missing_for_send_push_flow() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![
            NodeScheduleSpec::new("producer", vec!["input".to_owned()]),
            NodeScheduleSpec::new("worker", Vec::<String>::new()),
        ];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-push-autoch"),
        )
        .with_recursion_limit(6);

        let result = engine
            .run(
                &PushFlowRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("start"))],
            )
            .unwrap();

        assert_eq!(result.tasks_executed, 2);
        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("from_send"))
        );
    }

    #[test]
    fn errors_when_tasks_channel_payload_is_not_send_list() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert(
            "__pregel_tasks".to_owned(),
            Box::new(LastValue::new("__pregel_tasks")),
        );

        let node_specs = vec![NodeScheduleSpec::new("producer", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-invalid-tasks"),
        )
        .with_recursion_limit(3);

        let error = engine
            .run(
                &EchoRunner,
                None,
                config,
                vec![ChannelWrite::new("__pregel_tasks", json!({"bad": true}))],
            )
            .unwrap_err();

        match error {
            LoopError::Scheduler(SchedulerError::InvalidTasksChannelPayload {
                channel, ..
            }) => {
                assert_eq!(channel, "__pregel_tasks");
            }
            other => panic!("expected InvalidTasksChannelPayload, got {other}"),
        }
    }

    #[test]
    fn produces_deterministic_push_and_pull_task_ids_and_paths() {
        let run_once = || {
            let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
            channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
            channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

            let node_specs = vec![
                NodeScheduleSpec::new("producer", vec!["input".to_owned()]),
                NodeScheduleSpec::new("worker", Vec::<String>::new()),
            ];
            let engine = LoopEngine::new(channels, node_specs);
            let config = LoopConfig::new(
                crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-det-ids"),
            )
            .with_recursion_limit(6);

            engine
                .run(
                    &PushFlowRunner,
                    None,
                    config,
                    vec![ChannelWrite::new("input", json!("start"))],
                )
                .unwrap()
                .task_reports
                .into_iter()
                .map(|report| (report.task.id, report.task.path.to_path_string()))
                .collect::<Vec<_>>()
        };

        assert_eq!(run_once(), run_once());
    }

    #[test]
    fn untracked_value_can_drive_followup_tasks_but_is_not_checkpointed() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert(
            "scratch".to_owned(),
            Box::new(UntrackedValue::new("scratch", true)),
        );
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![
            NodeScheduleSpec::new("producer", vec!["input".to_owned()]),
            NodeScheduleSpec::new("consumer", vec!["scratch".to_owned()]),
        ];
        let engine = LoopEngine::new(channels, node_specs);
        let saver = InMemorySaver::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-untracked-flow"),
        )
        .with_recursion_limit(5);

        let result = engine
            .run(
                &UntrackedFlowRunner,
                Some(&saver),
                config,
                vec![ChannelWrite::new("input", json!("start"))],
            )
            .unwrap();

        assert_eq!(result.status, LoopStatus::Done);
        assert_eq!(result.tasks_executed, 2);
        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("secret"))
        );
        assert!(!result.checkpoint.channel_values.contains_key("scratch"));
    }

    #[test]
    fn persisted_writes_skip_untracked_channels_and_sanitize_send_payload() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert(
            "scratch".to_owned(),
            Box::new(UntrackedValue::new("scratch", true)),
        );
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("persist", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let saver = InMemorySaver::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new(
                "thread-untracked-persist",
            ),
        )
        .with_recursion_limit(3);

        let result = engine
            .run(
                &UntrackedFlowRunner,
                Some(&saver),
                config,
                vec![ChannelWrite::new("input", json!("start"))],
            )
            .unwrap();

        let saved = saver.get_tuple(&result.checkpoint_config).unwrap().unwrap();
        let pending_writes = saved.pending_writes.unwrap_or_default();

        assert!(
            !pending_writes
                .iter()
                .any(|write| write.channel == "scratch"),
            "untracked channel write should not be persisted"
        );
        assert!(
            pending_writes
                .iter()
                .any(|write| write.channel == "output" && write.value == json!("ok")),
            "regular writes must still be persisted"
        );

        let tasks_write = pending_writes
            .iter()
            .find(|write| write.channel == "__pregel_tasks")
            .expect("expected persisted tasks-channel write");

        let send_packet: SendPacket = serde_json::from_value(tasks_write.value.clone())
            .expect("tasks-channel write should be serialized SendPacket");

        let send_arg = send_packet
            .arg
            .as_object()
            .expect("send arg should be object");
        assert!(send_arg.contains_key("safe"));
        assert!(!send_arg.contains_key("scratch"));
    }

    #[test]
    fn injects_managed_values_into_pull_task_input() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert(
            "last_step".to_owned(),
            Box::new(LastValue::new("last_step")),
        );
        channels.insert(
            "remaining".to_owned(),
            Box::new(LastValue::new("remaining")),
        );

        let node_specs = vec![
            NodeScheduleSpec::new("managed", vec!["input".to_owned()]).with_input_channels(vec![
                "input".to_owned(),
                "is_last_step".to_owned(),
                "remaining_steps".to_owned(),
            ]),
        ];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-managed-input"),
        )
        .with_recursion_limit(2);

        let result = engine
            .run(
                &ManagedInputRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("go"))],
            )
            .unwrap();

        assert_eq!(result.status, LoopStatus::Done);
        assert_eq!(
            result.checkpoint.channel_values.get("last_step"),
            Some(&json!(true))
        );
        assert_eq!(
            result.checkpoint.channel_values.get("remaining"),
            Some(&json!(1))
        );
    }

    #[test]
    fn execution_context_supports_dynamic_read_and_send() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));
        channels.insert(
            "managed_out".to_owned(),
            Box::new(LastValue::new("managed_out")),
        );

        let node_specs = vec![NodeScheduleSpec::new("reader", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-runtime-read"),
        )
        .with_recursion_limit(2);

        let result = engine
            .run(
                &RuntimeCapabilityRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("hello"))],
            )
            .unwrap();

        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("hello"))
        );
        assert_eq!(
            result.checkpoint.channel_values.get("managed_out"),
            Some(&json!(true))
        );
    }

    #[test]
    fn execution_context_supports_dynamic_call() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));
        channels.insert("called".to_owned(), Box::new(LastValue::new("called")));

        let node_specs = vec![
            NodeScheduleSpec::new("caller", vec!["input".to_owned()]),
            NodeScheduleSpec::new("worker", Vec::<String>::new()),
        ];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-runtime-call"),
        )
        .with_recursion_limit(2);

        let result = engine
            .run(
                &RuntimeCapabilityRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("ping"))],
            )
            .unwrap();

        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("ping"))
        );
        assert_eq!(
            result.checkpoint.channel_values.get("called"),
            Some(&json!(true))
        );
    }

    #[test]
    fn exit_durability_persists_on_finish() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let saver = InMemorySaver::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-exit-durability"),
        )
        .with_durability(DurabilityMode::Exit)
        .with_recursion_limit(4);

        let result = engine
            .run_with_input(
                &EchoRunner,
                Some(&saver),
                config,
                LoopInput::Writes(vec![ChannelWrite::new("input", json!("hello"))]),
            )
            .unwrap();

        let saved = saver.get_tuple(&result.checkpoint_config).unwrap().unwrap();
        assert_eq!(
            saved.checkpoint.channel_values.get("output"),
            Some(&json!("hello"))
        );
    }

    #[test]
    fn async_durability_flushes_worker_and_orders_checkpoint_before_writes() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let saver = RecordingSaver::default();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("thread-async-durability"),
        )
        .with_durability(DurabilityMode::Async)
        .with_recursion_limit(4);

        let result = engine
            .run_with_input(
                &EchoRunner,
                Some(&saver),
                config,
                LoopInput::Writes(vec![ChannelWrite::new("input", json!("hello"))]),
            )
            .unwrap();

        let saved = saver.get_tuple(&result.checkpoint_config).unwrap().unwrap();
        assert_eq!(
            saved.checkpoint.channel_values.get("output"),
            Some(&json!("hello"))
        );

        let operations = saver.operations();
        for (idx, operation) in operations.iter().enumerate() {
            if let Some(checkpoint_id) = operation.strip_prefix("writes:") {
                let put_seen = operations
                    .iter()
                    .take(idx)
                    .any(|entry| entry == &format!("put:{checkpoint_id}"));
                assert!(
                    put_seen,
                    "writes for checkpoint '{checkpoint_id}' were persisted before checkpoint put"
                );
            }
        }
    }
}
