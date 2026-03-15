## LangGraph -> Deep Dive
LangGraph models your agent as a state machine -> a directed graph where:
- Nodes = things that happen (LLM calls, tool executions, decisions)
- Edges = transitions between nodes (fixed or conditional)
- State = a typed object that flows through the entire graph, getting updated at each node 

Every node receives the current state, does something, and returns an updated state. That's it. Everything else is built on that 
[START] → [call_llm] → [execute_tool] → [call_llm] → [END]
                              ↑_______________|  (loop back if not done)

## The State Object
This is the foundation. Everything in the graph reads from and writes and writes to this.
```py
from typing import TypedDict, Annotated
from langgraph.graph.message import add_messages

class TradingAgentState(TypedDict):
    # Messages accumulate - add_messages is a reducer that appends
    messages: Annotated[list, add_messages]

    # working data
    trades: list[dict]
    analysis: dict

    # Control flow
    current_step: str
    errors: list[str]
    confirmed: bool
```
Reducers are key. When a node returns {"messages": [new_msg]}, LangGraph doesn't replace messages -> it appends, because add_messages is a reducer. You define how each field gets updated when nodes write to it

This is how parallel nodes can safely write to shared state without trampling each other 

## Nodes
A node is just a function. Takes state, returns partial state update.
```py
async def fetch_trades(state: TradingAgentState):
    trades = await db.query(
        "SELECT * FROM trades ORDER by date DESC LIMIT 10"
    )
    return {"trades": trades} # only update this feild

async def call_llm(state: TradingaAgentState):
    response = await llm.invoke(state["message"])
    return {"messages": [response]} # appeneded via reducer

async def execute_tool(state: TradingAgentState):
    last_message = state["messages"][-1]
    tool_call = last_message.tool_calls[0]
    result = await tools[tool_call["name"]].ainvoke(tool_call["args"])
    return {"messages": [ToolMessage(result, tool_call_id=tool_call["id"])]}
```
Clean. Each node has one job. No side effects on other nodes 

## Edges -> Fixed and Conditional
Fixed edges - always go from A to B:
```py
graph.add_edge("fetch_trades", "call_llm")
```

Conditional edges - decide at runtime which node to go to next:
```py
def should_continue(state: TradingAgentState) -> str:
    last_message = state["messages"][-1]

    if last_message.tool_calls:
        return "execute_tool"   # LLM wants to call a took
    else:
        return "END"     # LLM gave a final answer

graph.add_conditional_edges(
    "call_llm", 
    should_continue,
    {
        "execute_tool": "execute_tool",   # if returns "execute_tool", go here
        "END": END   # if returns "END", terminate
    }
)
```
This is where the ReAct loop lives - the condition after call_llm checks if the model wants to use a tool, and loops back if so.

## Bulding the full Graph
```py
from langgraph.graph import StateGragh, START, END

# 1. Create graph with your state type 
graph = StateGraph(TradingAgentState)

# 2. Add nodes
graph.add_node("fetch_trades", fetch_trades)
graph.add_node("call_llm", call_llm)
graph.add_node("execute_tool", execute_tool)

# 3. Add edges
graph.add_edge(START, "fetch_trades")
graph.add_edge("fetch_trades", "call_llm")
graph.add_conditional_edges(
    "call_llm",
    should_continue,
    {
        "execute_tool": "execute_tool",
        "END": END
    }
)
graph.add_edge("execute_tool", "call_llm")

# 4. Compile
app = graph.compile()
```
Now you have a complete agent that can:
1. Fetch trades
2. Analyze them with LLM
3. Execute tools if needed
4. Loop until done

## Running the Graph
```py
async for event in app.astream_events(
    {"messages": [HumanMessage("Analyze recent trades")]},
    version="v1"
):
    print(event)
```
This is where the magic happens. The graph executes nodes in the correct order, handles parallelism, and manages the state.

## Checkpointing - How LangGraph Handles State Persistence

This is one of LangGraph's killer features. Every state transition can be checkpointed to a database. This means:
- Resume a long-running task after a crash
- Human-in-the-loop -> pause mid-graph, wait for user input, resume
- Time travel -> rewind to any previous state and replay from there
- Fork -> branch from a past state and try a different path

```py
from langraph.checkpoint.sqlite import SqliteSaver

checkpoint = SqliteSaver.from_conn_string("trading_agent.db")
app = graph.compile(checkpointer=checkpointer)

# Every run needs a thread_id - this is the "coversation"
config = {"configurable": {"thread_id": "user_123_session_1"}}

# Run
result = await app.ainvoke(input, config=config)

# Later - resume the same thread
result = await app.ainvoke(new_input, config=config)

# Inspect state at any point
state = await app.aget_state(config)

# Rewind to a previous checkpoint
history = [s async for s in app.aget_state_history(config)]
past_state = history[3] # 3 steps ago
```
This is what makes LangGraph genuinely production-readfy - not just a research toy.

# Humain-in-the-loop -> Interrupts
This is where LangGraph's graph model really shines. You can interrupt the graph at any node, wait for hum input, then resume.

```py
# Compile with interrupt points
app = graph.compile(
    checkpointer=checkpointer,
    interrupt_before=["execute_trade"] # pause before this node
)

# First run - executes until it hits execute_trade, then stops
result = await app.ainvoke(input, config=config)

# Agent is paused. Show user what it wants to do.
state = await app.aget_state(config)
pending_action = state.value["pending_trade"]
print(f"Agent wants to execute: {pending_action}")

# User approves
if user_approved:
    # Resume - graph continues from where it stopped
    result = await app.ainvoke(None, config=config)
else:
    # User rejected - update state and resume with a flag
    await app.aupdate_state(config, {"confirmed": False})
    result = await app.ainvoke(None, config=config)
```
The agent doesn't een know it was paused. From its perspective, it's just work up with the state it left off with

## Parallel Nodes (Branches)
LangGraph supports forking and joining - run multiple nodes simulttaneously, merge results.

```py
# Fork into parallel branches
graph.add_edge("fetch_trades", "analyze_positions")
graph.add_edge("fetch_trades", "get_market_conditions")
graph.add_edge("fetch_trades", "analyze_timing")

# All three run in parallel, then merge into synthesize
graph.add_edge("analyze_positions", "synthesize")
graph.add_edge("get_market_conditions", "synthesize")
graph.add_edge("analyze_timing", "synthesize")
```

LangGraph detects that synthesize has multiple incoming edges and waits for all branches to complete before running it. The state reducers handle merging the parallel outputs safely.

## Subgraphs - Agents as modules

You can nest graphs inside graohs. A subgraph looks like a single node to the parent.
```py
# Build a specialized analyst subgraph
analyst_graph = StateGraph(AnalystState)
analyst_graph.add_node("retrieve", retrieve_data)
analyst_graph.add_node("analyze", run_analysis)
analyst_graph.add_edge("retrieve", "analyze")
analyst_app = analyst_graph.compile()

# Use it as a node in a larger graph
main_graph.add_node("run_analysts", analyst_app)
```
This is how i build clean multi-agent systems - each agent is its own complied graph, composed into an orchestrating parent graph.


## Concepts 

* State 
Think of a state as a shared whiteboard that every node can read and write to.
It's just a typed object. Everyone in the graph looks at the same whiteboard.

```py
class State(TypedDict):
    messages: list
    trades: list
    analysis: str
```

* Nodes
A node is just a function that does on job
It reads the whiteboard, does something, writes back to the whiteboard

```py
def fetch_trades(state):
    trades = db.get_last_10_trades()
    return {"trades": trades} # update the whiteboard
```
That's it. One job. Read state, return update

* Edges 
Edges are the arrows connecting nodes - they decide what runs next.

Two kinds:
Fixed - always go to same place:
```py
graph.add_edge("fetch_trades", "analyze")
# fetch_trades ALWAYS goes to analyze. No decision.
```

Conditional - look at the state, decide where to go:
```py
def what_next(state):
    if state["needs_more_data"]:
        return "fetch_mpre"
    return "respond"
graph.add_conditional_edges("analyze", what_next)
```

Put it together 
[START] -> [fetch_trades] -> [analyze] -> [respond] -> [END]

* State = the whiteboard everyone shares
* Nodes = workers who each do one jo on that whiteboard
* Edges = the hallways connecting workers, telling them who's next

Every node gets the full whiteboard, does its job, passes the updated whiteboard down the hallway to the next node.

