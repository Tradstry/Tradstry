mod map;

/// Runtime IO mapping helpers.
///
/// This module exposes input-command mappers and Python-compatible output mappers
/// used by loop dual stream parity mode.
pub use map::{
    DebugPayloadType, IoMapError, OutputChannels, OutputWriteGate, PendingWriteTuple,
    TaskOutputWrites, map_checkpoint_payload, map_command, map_command_to_writes,
    map_debug_wrapper, map_input_writes, map_output_updates, map_output_values, map_task_payload,
    map_task_result_payload,
};
