# How Checpoints work - Simple Explanation
Checkpoints are like save points in a video game -> they let you pause your execution, save everything, and come back later to continue where you left off.

# The Core Concept: Save & Resume
Think of running a complex workflow that takes hours:

Step 1: Get user input
Step 2: Call AI model
Step 3: Process results
Step 4: Call another AI... CRASH!

Without checkpoints: Start over from Step 1, with checkpoints: Resume from Step 3

# What Gets Saved in a Checkpoint?
A checkpoint is a snapshot of your entire graph state:
```json
Checkpoint {
    id: "1234-56789-9abc",  // Unique identifier
    ts: "2024-03-04T10:30:00Z", // Timestamp

    // The actual data
    channel_values: {
        "input": "hello",
        "messages": ["msg1", "msg2"],
        "output": "results",
    },

    // Version tracking (for scheduler)
    channel_versions: {
        "input": 5,
        "messages": 3,
        "output":, 2
    },

    // What each node has seen
    versions_seen: {
        "node1": {"input": 5},
        "node2": {"messages": 3}
    },

    // Queued messages to other graphs
    pending_sends: [...]
}
```

# The Design: Three layers

1. Types (What to Save)
- Checkpoint - The snapshot itself
// Everything you need to restore execution
- channel_values: What's in each mailbox
- channel_versions: Version numbers
- versions_seen: What each node has processed
- pending_sends: Messages waiting to be sent

- CheckpointConfig - Where to save/load
// Identifies a specific execution thread
thread_id: "user-123-session"
checkpoint_ns: "subgraph-1"  // optional namespace
checkpoint_id: "abc-123"     // Optional specific checkpoint

- CheckpointMetadata - Extra info about the checkpoint 
soure: Input/Loop/Update/Fork.  // How was it created?
step: 5.    // Which step number
parents: {"parent": "xyz"}     // Parent checkpoint IDS 
run_id: "run-456"

- PendingWrite - Writes that haven't been applied yet
task_id "task-1"
channel: "output"
value: {"result": 42}
task_path: "pull:step1:node1"

2. Saver Trait (How to Save)
The `CheckpointSaver` trait defines the interface - any storage backend must implement these methods:

```rs
trait CheckpointSaver {
    // core operations
    fn get(config) -> Checkpoint  // Load a checkpoint 
    fn put(checkpoint, metadata) -> Config  // Save a checkpoint
    fn put_writes(writes)     // Save pending writes 
    fn list(query) -> Vec<Checkpoint>      // Query checkpoint history

    // Maintenance operations 
    fn delete_thread(thread_id)     // Delete all checkpoints for a thread
    fn copy_thread(source, target)  // Copy thread history
    fn prune(thread_ids, strategy)  // Clean up old checkpoints 

    // Async versions (optional)
    async fn aget(config) -> Checkpoint
    async fn aput(checkpoint) -> Config
    //  ... etc
}
```
Why a trait? So you can swap storage backends without changing your code! 

3. Backends (Where to Save)
Three implementations of same interface:

- InMemorySave - "Save in RAM"
```rs
// Uses HaspMap in memory
// Fast but lost when program exits
// Perfect for: Testing, development

let saver = InMeorySaver::new();
```

- SqliteSaver - "Save to local file"
```rs
// Uses SQLite database file
// Durable, singl-process
// Perrfect for: Desktop apps, single-server deployments
let saver = SqliteSaver::new("checkpoints.db")?;
```

- PostgresSaver - "Save to database server"
```rs
// Uses PostgreSQL database
// Durable, multiple-process, distributed
// Perfect for: Production, cloud deployments
let saver = PostgresSaver::new("postgres://...")?;
```


# How it Works in Practice

- Saving a Checkpoint
```rs
// 1. Runtime finishes a step
let checkpoint = create_checkpoint(
    &previous_checkpoint,
    Some(&channels),  // Current channel state
    step,             // Step number
    None              // Auto-generate ID
)?;

// 2. Create metadata
let metadata = CheckpointMetadata {
    source: CheckpointSource::Loop,
    step: Some(5),
    parents: parent_map(&config),
    ..Default::default()
};

// 3. Save it
let new_config = saver.put(
    &config,
    checkpoint,
    metadata,
    new_versions  // Which channels changed
)?;

// 4. Save pending writes
saver.put_writes(
    &new_config,
    &writes,      // What nodes wrote
    &task_id,     // Which task
    &task_path    // Task path for debugging
)?;
```

- Loading a Checkpoint
```rs 
// 1. Specify which checkpoint to load
let config = CheckpointConfig::new("user-123-session")
    .with_checkpoint_id("abc-123");  // Optional: specific checkpoint

// 2. Load it
let tuple = saver.get_tuple(&config)?;

// 3. Restore state
let checkpoint = tuple.checkpoint;
let channels = restore_channels(&channel_specs, &checkpoint)?;

// 4. Continue execution from this point!
```

# Key Design Concepts
* 1. Thread-based Organizaton
thread_id: "user-123"
  ├─ checkpoint-1 (step 0)
  ├─ checkpoint-2 (step 1)
  ├─ checkpoint-3 (step 2)
  └─ checkpoint-4 (step 3)
Each "thread" is an independent execution history. You can have multiple threads running simultaneously.

* 2. Parent-Child Relationships
Main Graph Checkpoint
  └─ parents: {}
      ↓
  Subgraph Checkpoint
    └─ parents: {"": "main-checkpoint-id"}
        ↓
    Nested Subgraph Checkpoint
      └─ parents: {"": "subgraph-checkpoint-id"}
Checkpoints can have parent checkpoints (for nested graphs).

* 3. Pending Writes 
Step 3 completes:
  - Node A writes to "output"
  - Node B writes to "messages"
  
These writes are saved as "pending" until Step 4 applies them.

This lets you see what happened in each step, even before it's applied

* 4. Reserved with Channels
Some channels have special meaning:
"__error__"     → write_idx = -1  (errors)
"__scheduled__" → write_idx = -2  (scheduled tasks)
"__interrupt__" → write_idx = -3  (interrupts)
"__resume__"    → write_idx = -4  (resume data)
These get special ordering in the database

* 5. Checkpoint ID Generation
// IDs are sortable by time
next_checkpoint_id(step) -> "2024-03-04T10:30:00.123Z-step5"

// Monotonic ordering ensures:
checkpoint_1.id < checkpoint_2.id < checkpoint_3.id

You can sort checkpoints by ID to get chronological order


# Complete flow
┌─────────────────────────────────────────┐
│  RUNTIME EXECUTES STEP                  │
│  - Nodes run                            │
│  - Channels update                      │
│  - State changes                        │
└─────────────────┬───────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│  CREATE CHECKPOINT                      │
│  - Snapshot channel values              │
│  - Copy version numbers                 │
│  - Copy versions_seen                   │
│  - Generate unique ID                   │
└─────────────────┬───────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│  SAVE VIA SAVER                         │
│  - saver.put(checkpoint, metadata)      │
│  - saver.put_writes(pending_writes)     │
└─────────────────┬───────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│  STORAGE BACKEND                        │
│  - InMemory: Store in HashMap           │
│  - SQLite: INSERT INTO checkpoints      │
│  - Postgres: INSERT INTO checkpoints    │
└─────────────────────────────────────────┘

Later...

┌─────────────────────────────────────────┐
│  LOAD CHECKPOINT                        │
│  - saver.get_tuple(config)              │
└─────────────────┬───────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│  RESTORE STATE                          │
│  - Recreate channels from values        │
│  - Restore version numbers              │
│  - Restore versions_seen                │
└─────────────────┬───────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│  RESUME EXECUTION                       │
│  - Continue from saved step             │
│  - Apply pending writes                 │
│  - Run next nodes                       │
└─────────────────────────────────────────┘


