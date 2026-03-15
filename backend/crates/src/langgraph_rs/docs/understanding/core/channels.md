# How Channels work - Simple Explanation
Think of channels like smart mailboxes in a graph. Each node can read from and write to these mailboxes, but the maiboxes have different rules about how they handle messages.

# The Core Design Concept
The problem: In a graph workflow, nodes need to pass data to each other. But different situations need different behaviours:

* Sometimes you want the latest value only
* Sometimes you want to collect all values
* Sometimes you want to wait for multiple inputs before proceeding
* Sometimes you want to add numbers together

The solution: Create different types of "mailboxes" (channels) with different, ut they all follow the same interface (the `Channel` trait)

# The Channel Trait - The Contract
Every cchannel must implement these operations:
```rs
trait Channel {
    fn get() -> Value               // Read the current value
    fn update(values) -> bool  // Write new values, return true if changed
    fn consume() -> bool           // Mark as "read (for triggers)
    fn finish() -> bool           // Finalize for this step
    fn checkpoint() -> Value      // Save state to disk
    fn from_checkpoint() -> Self    // Restore from disk
}
```
This is like saying "every mailbox must let you read mail, put mail in, mark it as read, and save/restore its contents."

# Channel Categories & How They Work

1. Basic Value Channels (Simple storage)
LastValue - "Only one letter at a time, please"
```rs
// stores exactly ONE value per step
// Rejects ultiple updates in same step
channel.update(&[json!(1)])    // Ok
channel.update(&[json(1), json(2)])    // Error: "only one value per step"
```
Use when: Each node should write exactly once per step

AnyValue - "I'll take the last one you give me"
```rs
// Takes multiple values, keeps the last one
// Empty update clears it 
channel.update(&[json!(1), json!(2), json(3)])   // Keeps json!(3)
channel.update(&[])
```
Use when: You want flexibility but only care about the final value

EphemeralValue - "Temporary storage, clears automatically"
```rs
// Like AnyValue but designed for temporary data
// Has guard mode to enfore single update
let channel = EphemeralValue::new("temp", guard=true);
```
Use when: Temporary calculations that don't need to persist

2. Aggregation Channels (Combining Values)
- BinaryOperatorAggregate - "I combine values using math/logic"
```rs
// Reduces values using a custom function
let sum = BinaryOperatorAggregate::add_numeric("total");
sum.update(&[json!(1), json!(2), json!(3)])   // Result: 6

// Custom operator - append to list
let append = Arc::new(|left, right| {
    let mut arr = left.as_array().unwrap().clone();
    arr.push(right.clone());
    Ok(Value::Array(arr))
});
let list = BinaryOperatorAggregate::new("items", append);
list.update(&[json!(1), json!(2), json!(3)])   // Result: [1, 2, 3]
```

- Special Feature - Overwrite
```rs
// Normal: adds to sum
sum.update(&[json!(1), json!(2)])    // sum = 3

// Overwrite: replaces sum completely
sum.update(&[Overwrite::new(json!(100).into())]) // sum = 100 (ignore previous)
```
Use when: You need to accumulate/reduce values, but sometimes want to reset

- Topic - "Collect everything into a list"
```rs
// Collects all values into an array
// Flattens nested arrays automatically
let topic = Topic::new("messages", accumulate=true);
topic.update(&[json!(1), json!([2, 3]), json!(4)]); // Result: [1, 2, 3, 4]  (notice [2,3] was flattened)
```
Use when: Building a list of messages/events


3.SYNCHRONIZATION CHANNELS (Coordination)

- NamedBarrierValue - "Wait for everyone before opening"
```rs
// Like a door that needs 3 keys to open
let barrier = NamedBarrierValue::new("join", ["alice", "bob", "charlie"]);

barrier.update(&[json!("alice")]) // Not ready yet
barrier.get() // ERROR: EmptyChannel

barrier.update(&[json!("bob"), json!("charlie")]) // All keys received!
barrier.get() // OK: returns null (meaning "ready")
```
Use when: Multiple nodes must complete before proceeding (fan-in pattern)

- NamedBarrierValueAfterFinish - "Wait for everyone AND a finish signal"
```rs
// Same as above but needs explicit finish() call
barrier.update(&[json!("alice"), json!("bob")])
barrier.finish() // Now it's ready
```
Use when: You need explicit control over when the barrier releases


4. SPECIAL CHANNELS (Unique Behaviors)

- UntrackedValue - "I never save to disk"
```rs
// Stores value but checkpoint() returns None
// Perfect for API keys, temporary data, sensitive info
let secret = UntrackedValue::new("api_key", guard=true);
secret.update(&[json!("sk-12345")])
secret.checkpoint() // Returns None - never persisted!
```
Use when: Sensitive data that shouldn't be saved

- LastValueAfterFinish - "Hidden until finish() is called"
```rs
// Value is invisible until finish() is called
channel.update(&[json!(42)])
channel.get() // ERROR: EmptyChannel (not finished yet)

channel.finish()
channel.get() // OK: returns 42
```
Use when: You want to hide intermediate results until computation is complete


# Key Design Principles

1. Versioning System Every time a channel's state changes, its version number increases:

Step 1: channel version = 0 (empty)
Node writes → channel version = 1
Node writes again → channel version = 2

The scheduler uses versions to know which nodes need to run (only run if channel version changed since last time node saw it).

2. Update Returns Boolean
```rs
fn update(&mut self, values: &[Value]) -> Result<bool, ChannelError>
//                                                 ^^^^
//                                                 true = state changed
//                                                 false = no change
```
This tells the scheduler whether to bump the version number.

3. Checkpoint/Restore Pattern
```rs
// Save state
let saved = channel.checkpoint()?; // Returns JSON

// Later... restore state
let restored = channel.from_checkpoint(Some(&saved))?;
```
This enables pause/resume of entire graph execution.

4. Consume for Triggers
```rs
// Trigger channels get "consumed" after reading
channel.consume() // Marks as read, may clear value
```
This prevents nodes from re-triggering on the same data.

# How They Work Together in a Graph

```rs
// Build a graph with different channel types
let mut builder = GraphBuilder::new();

// Input: simple value
builder.add_channel("input", Box::new(LastValue::new("input")))

// Accumulator: sum numbers
builder.add_channel("total", Box::new(BinaryOperatorAggregate::add_numeric("total")))

// Barrier: wait for 2 nodes
builder.add_channel("sync", Box::new(NamedBarrierValue::new("sync", ["a", "b"])))

// Output: collect results
builder.add_channel("results", Box::new(Topic::new("results", true)))
```
Execution Flow:

Node writes to "input" → version bumps → triggers next node
Node writes to "total" → accumulates values → version bumps
Nodes write to "sync" → barrier waits for both → version bumps when complete
Nodes write to "results" → collects into array → version bumps
The scheduler watches all these version changes and decides which nodes to run next!
