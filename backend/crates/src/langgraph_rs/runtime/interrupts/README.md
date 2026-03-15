# `runtime/interrupts` Purpose

This folder implements interrupt selection/policy for pause-like runtime behavior.

## Responsibilities
- Represent interrupt selector configuration (`none`, `all`, node set).
- Compute whether a planned task batch should interrupt.
  - Parity gate: interrupts are only allowed when any channel version advanced since
    the last recorded interrupt checkpoint view.
- Derive deterministic interrupted node names for stream events.
