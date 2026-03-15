# How Scheduler Works - Simple Explanation

1. Planning - "Who should run next?"

2. Applying - "Update the state after nodes finish"

1. Planning (plan.rs) - "Who Runs Next?"
Think of the scheduler as a teacher calling on students who raised their hands

How it Decides Who Runs:

* Version Tracking System:
Every channel has a version number (like a timestamp):
- Channel starts a version 0
- Someone writes to it -> version becomes 1 
- Someone writes again -> version becomes 2

Every node remembers what version it last saw:
- Node A last saw chnnel "input" at version 1
- Channel "inpit" is now at version 3
- Node A needs to run! (it missed versions 2 and 3)

* The Planning Algorithm:
```rs 
fn plan_next_tasks() {
    // 1. Check which channels were updated
    for each updated_channel {
        // 2. Find nodes that listen to this channel
        let nodes = trigger_to_nodes[updated_channel];

        // 3. For each node, check if it needs to run
        for node in nodes {
            let current_version = channel.version;
            let last_seen_version = node.versions_seen[channel];

            if current_version > last_seen_version {
                // Node hasn't seen the latest data!
                planned_tasks.push(node);
            }
        }
    }

    return planned_tasks; // These nodes will run
}
```

# Two Types of Takes:

* Pull Tasks (normal execution):
Node is triggered because a channel it watches got updated 
Example: "inpit" channel changed -> run "process_input" node

* Push Tasks (dynamic routing):
A node sent a message to another node directly
Example: Node A says "send this data to node B"
Uses special "__pregel_tasks" channel

2. Applying (apply.rs) - "Update Everything After Nodes Run"
Think of thus as cleaning up after a meeting - updating notes, marking tasks complete, preparing for the next round.

The Apply Process (Step by Step):

* Step 1: Sort Tasks Deterministically
```rs
// Always process tasks in the same order (for consistency)
tasks.sort_by(task.path + task.name + task.id)
```

* Step 2: Update "Versions Seen"
```rs
// Mark that nodes have now seen the latest channel versions
for task in tasks {
    node.versions_seen[channel] = current_version;
}
```

* Step 3: Consume Trigger Channels
```rs
// Mark Trigger chanels as "read"
for trigger_channel in task.triggers {
    channel.consume(); // May clear the channel
    bump_version(channel);
}
```

* Step 4: Apply Writes to Channels
```rs
// Group all writes by channel
grouped_writes = {
    "output": [value1, value2, value3],
    "messages": [msg1, msg2]
}

// Update each channel
for (channel_name, values) in grouped_writes {
    if channel.update(values) {
        bump_version(channel);
        mark_as_updated(channel);
    }
}
```

* Step 5: Bump step (Empty updates)
```rs
// For channels that weren't writtne to, send empty update
// This lets channels advance their internal state
for channel in all_channels {
    for !channel.was_updated {
        channel.update([]); // Empty update
    }
}
```

* Step 6: Finish Channels (if Done)
```rs
// If no more nodes will trigger, finalize channels
if no_more_triggers {
    for channel in all_channels {
        channel.finish(); // Finalize state
    }
}
```

# Key Design Concepts

1. Determinism - Same Input = Same Output
The scheduler ALWAYS processes things in the same order:
- Tasks sorted by path/name/id
- Channels processed alphabetically
- Version numbers increment predictable

Why? So yo can debug, reoply and test reliably!

2. Version-Based Triggering
Instead of "run everything every time":
- Track what version each node last saw
- Only run nodes that missed updates
- Efficient and precise

3. Reserved Channels
Some channel names are special:
- "__pregel_tasks" - for push tasks (dynamic routing)
- "__return__" - for node return values
- "__resume__" - for resuming after interrupt
- "__error__" - for error handling

These are filtered out during normal write processing

4. Trigger-to-Nodes Mapping
```json
// Pre-computed map for fast lookups
trigger_to_nodes = {
    "input": ["node1", "node2"],
    "messages": ["node3"],
    "output": ["node4"]
}

// When "input" updates, instantly know to run node1 and node2
```


# The Complete Flow
┌─────────────────────────────────────────────┐
│  SCHEDULER CHECKPOINT (State)               │
│  - channel_versions: {"input": 5, ...}      │
│  - versions_seen: {"node1": {"input": 4}}   │
│  - updated_channels: ["input", "output"]    │
└─────────────────────────────────────────────┘
                    ↓
        ┌───────────────────────┐
        │   PLANNING PHASE      │
        │  "Who should run?"    │
        └───────────────────────┘
                    ↓
    1. Check updated_channels
    2. Find nodes triggered by those channels
    3. Check if node.versions_seen < channel.version
    4. Create PlannedTask for each triggered node
                    ↓
        ┌───────────────────────┐
        │  EXECUTION PHASE      │
        │  (Runtime runs nodes) │
        └───────────────────────┘
                    ↓
        ┌───────────────────────┐
        │   APPLYING PHASE      │
        │  "Update everything"  │
        └───────────────────────┘
                    ↓
    1. Sort tasks deterministically
    2. Update versions_seen
    3. Consume trigger channels
    4. Apply writes to channels
    5. Bump step (empty updates)
    6. Finish channels if done
    7. Update checkpoint
                    ↓
        ┌───────────────────────┐
        │  REPEAT (next step)   │
        └───────────────────────┘
