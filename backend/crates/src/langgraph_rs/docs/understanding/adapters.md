# How Adapters work - Simple Explanation
Adapters is the "USB port" between LangGraph runtime and external AI crates: core runtime only knows a generic runner contract (LoopNodeRunner) and generic result/error types, not Rig/LangChain types (types.rs, execution.rs).

Each adapter node implements one small contract, AdapterNode::execute(input_json, context) -> NodeExecutionResult, and FnAdapterNode lets you build one from a closure, so wiring providers is easy (node.rs)

AdapterRegistry is a name-to-node map with safety checks (no empty names, no duplicates), which is classic plugin registry design (registry.rs).

AdapterRunner is the dispatcher: runtime calls it with a node_name, it looks up the adapter, converts runtime ExecutionContext into a smaller AdapterContext, then executes the adapter (runner.rs, types.rs)

Rig and LangChain modules are thin convenience wrappers on top of that same contract (`from_text_handler`, `from_value_handler`, `from_message_handler`) that convert provider outputs into channel writes, and normalize provider failures into `NodeExecutionError` ([rig/node.rs](/Users/user/LangGraph/src/langgraph_rs/adapters/rig/node.rs:28), [langchain_rust/node.rs](/Users/user/LangGraph/src/langgraph_rs/adapters/langchain_rust/node.rs:40)). 

So the design concepts are: dependency inversion (core depends on traits, not providers), adapter pattern (translate provider I/O), registry pattern (dynamic lookup by name), and boundary normalization (JSON + unified errors), which keeps the engine deterministic, testable, and easy to extend.