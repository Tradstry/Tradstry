# `runtime/io` Purpose

This folder provides deterministic runtime input/output mapping helpers aligned with Python Pregel `_io.py` semantics.

## Responsibilities
- Map `Command` inputs into pending writes (`map_command`).
- Map validated input write chunks into channel writes (`map_input_writes`).
- Map runtime state into Python-compatible stream chunks:
  - `map_output_values`
  - `map_output_updates`
  - `map_task_payload`
  - `map_task_result_payload`
  - `map_checkpoint_payload`
  - `map_debug_wrapper`

## Notes
- Parent-graph command routing from loop input remains intentionally deferred.
- Output mapping is additive to the structured runtime lifecycle event stream.
- Debug wrapper payloads follow Python-style shape:
  - `{step, timestamp, type, payload}` with `type in {"task","task_result","checkpoint"}`.
