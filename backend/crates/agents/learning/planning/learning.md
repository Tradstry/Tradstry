## Planning in Agents - Deep Dive 
Planning is how an agent takes a vague goal and figures out what to actually do. It's the difference between an agent that trashes around randomly and that makes real progress

# The core problem 
LLMs are good at the bext step. They're bad at the 47th step 
Without planning, an agent just vibes its way though a task -> reactive, no structure, easily derailed. Planning gives it stucture, foresight, and recoverability.

# Planning strategies 

* Chain of Thought (CoT) -> "Think before you act"
The simplest form. Just tell the model to reason step by step before doing anything. 

User: Analze my last 10 trades and find patterns 

Agent thinks:
"First i need to retrieve the trades. Then i need to look at entry/exit points. Then check position sizes. Then look for emotional patterns like revenge trading. Thw summarize"

-> Then starts executing

- Good for simple, linear tasks. Bad for anything with branching, uncertainty, or long horizons. The problem: The plan lives inside on generation. The model can forget it 5 steps later.

* ReAct -> "Think, Act, Observe, Repeat"
The workhorse of most agent systems today. Interleaves reasoning and action in a loop 

Thought: I need to get the last 10 trades first 
Action: query_database(SELECT * FROM trades ORDER BY date DESC LIMIT 10)
Observation: [returns 10 trades]

Thought: Now i see 7 of 10 trades were losses. I should look at position sizing 
Action: calculation_position_sizes(trades)
Observation: avg position was 40% of portfolio - way too high 

Thought: This looks like overleveraging. Let me check if losses cluster around volatile days 
Action: get_market_volatility(dates=[...])
Observation: 6 of 7 losses happened on high-VIX days 

Thought: I have enough t give a clear answer now
Action: respond(...)

Good for: Most asks. Ground in reality because each step observes actual results before proceeding. Bad for: Tasks that need real upfront planning before touching anyhting The problem: Greedy -> always takes the locally reasonable next step, not necessarily the globally optimal one. Can get tunnel-visioned

* Plan-and-Execute -> "Plan everything, then do it"
Separate the planning from the execution. Two distinct phases. 

Phase 1 -> Planner (LLM call):
Goal: "Find mistakes in my last 10 trades"

pLAN:
- Retrieve last 10 trades from DB
- Calculate win/loss ratio
- Analyze position sizes per trade
- Cross-reference with market conditions on those days
- Check entry timing vs intended strategy
- Identify emotional patterns (revenge trades, FOMO entries)
- Synthesize findings into actionable feedback 

Phase 2 -> Executor (separate LLM or same, step by step):
Execute step 1 -> observe -> execute step 2 -> observe -> ...

Good for: Complex tasts where you want a coherent strategy upfrontend. Easier to show the user "here's what i'm going to do" before doing t. Bad for: Tasks were you can't know what step 4 looks like until i see the result of step 2. The problem: tHE PLAN GOES STALE. Real execution hits surprises that the planner didn't anticipate, and a rigid plan can't adapt. 

Solution: Re-planning. When execution hits something unexpected, call the planner again with the new information and regenerate the remaining steps

Executing step 3 ...
Observation: "No position size data available before 2024"
-> Trigger re-plan: update steps 3-7 given this contraint 

* Tree of Thoughts (ToT) -> "Explore mutliple paths"
Instead of one linear plan, generate multiple possible next steps and evaluate which is most promising. Like a search tree

Current state: "I have the 10 trades retrieved"

Branch A: Analyze by win/loss 
-> sub-branch A1: Look at time of day '
-> Sub-branch A2: Look at position size 

Branch B: Analyze by market conditions 
-> Sub-branch B1: VIX levels
-> Sub-branch B2: Trend vs. ranging market

Branch C: Analyze by emotional markers 
-> Sub-branch C1: Trades after losses (revenge?)
-> Sub-branch C2: Trade frequency spikes

Evaluate each branch: which is most liekly to yield insight?
-> Pick best, continue, prune the rest 

Good for: Tasks where the right approach isn't obvious upfrontend. Creative problem solving. Reasearch. Bad for: Most practical agent tasks - it's expensive (many LLM calls) and slow. The problem Combinatorial explosion. I need smart pruning or it blows up 

* LLM-as-Planner + Specialized Executors 
A more architectural pattern than a reasoning strategy. The planner LLM is onyl responsible for what to do. Specialized sub-agents or tools handle how to do it. 

Planner (orchestrator):
"To analyze trading mistakes i need:
-> trading_analyst_agent: find patterns in the data 
-> risk_agent: evalute position sizing
-> market_context_agent: pull conditions on those dates"

Each sub-agent runs independently, returns structured results 
Planner synthesizes everything into final answer 

This is multi-agent planning -> covered more in the multi-agent section, but the planning concept is: decompose into specialized converns, parallelize where possible at the end 

## Key Planning concepts 
* Goal Decomposition 
Breaking a goal into subgoals. The art is in finding the right level of granularity

Too coarse:
1. Analyze trades
2. Find mistakes <- how?? this is the whole problem

Too granular:
1. Ope database connection
2. Write SQL query
3. Set LIMIT to 10
4. Set ORDER BY to date
... <- the model is micromanaging itself 

Right level:
1. Rertieve last 10 trades with full metadate 
2. Identify losing trades and their characteristics 
3. Find commmon patterns acoss losses
4. Compare against winning trades
5. Generate specific, actionable findings

## Task Graphs vs. Task Lists 
Many think of plans as lists (linear). But real tasks are graphs (some things can happen in parallel, some depend on others)

Linear(slow):
retrieve trades -> analyze positions -> get market data -> analyze timing -> synthesize

Graph (faster)
retrieve trades -> analyze position ----|
                 |-> get market data ------> synthesize 
                 |-> analyze timing |
If the execution layer supports it, parallelizing independent steps is a huge win. LangGraph is built around this -> tasks as nodes in a graph, edges as dependencies (study graph database to learn in-depth how this works)

## Plan Validation
Before executing, snaity check the plan:
- Does every step have the inputs it needs?
- Are there any steps that will definitely fail? (e.g., "get data from API X", but the agent has no API X tool)
- Is the plan actually necessary or is it overcomplicating a simple task?

Some systems have a separate "critic" LLM call that reviews the plan before execution starts 


### Dynamic Replanning
This is critical for real-world reliability. Plans will break. The question is how gracefully. 
Triggers for replanning:
- A tool call fails or returns unexpected results
- A step reveal that earlier assumptions were wrong
- The task turns out to be bigger than anticipated
- A step completes early making subsequent steps unnecessary 

```py
def execute_plan(plan, state):
    for step in plan:
        result = execute_step(step)
        if result.unexpected:
            plan = replace(original_goal, state, result) # regenerate remaining steps
            state.update(result)
```

### The Meta-Problem: When to Plan
Not every task needs explicit planning. Calling a planner LLM adds latency and cost.

Simple query: "what was my last trade?"
-> Just do it. No planning needed. ReAct handles it in 1-2 steps.

Complex task: "review my trading performance this quarter, identify weaknesses, and suggest a revised strategy"
-> Plan first. This has multiple distinct phases and will take 10+ steps.

A good heuristic: plan when the task has more than ~3 non-obvious steps or when order of operations matters significantly.

## Failure Modes in Planning 

* Over-planning -> agent makes a beautiful 15-step plan then fails on step 2 and has no idea what to do.

* Under-planning -> agent just starts doing stuff, gets 8 steps in, realizes it needed info from step 1 that it didn't collect.

* Plan hallucination -> agent plans to use a tool it doesn't have, or assumes data exits that doesn't

* Stale plans -> agent commits to the plan even when new evidence shows it's wrong. Rigidity kills 

* Infinite replanning -> agent keeps hitting obstacles, keeps replanning, never actually executes anything useful. I need a max replan limits 

## What good planning looks like in practice 

For the trading agent example, a solid plan-and-execute flow would be:

User: "What mistakes did i make in my last 10 trades?"

Planner:
Goal: Identify trading mistales
Constraints: Last 10 trades only, be specific not generic

Plan:
1. [DB] Fetch last 10 trades with: entry/exit price, size, date, P&L, instrument
2. [CALC] Compute: win rate, avg winner, avg loser, largest loss 
3. [DB] Fetch market conditions (VIX, trend) for each trade date 
4. [ANALYZE] Cross-reference losses with market conditions
5. [ANALYZE] Check position sizing consistency 
6. [ANALYZE] Check if any trades violated user's stated strategy rules 
7. [SYNTHESIZE] Group findings into mistake categories 
8. [RESPOND] Present with specific trade examples, not generalities 

Parallel opportunities: steps 2 and 3 can run simulataneously 

Executor: runs each step, adapts if something unexpected comes back


## The bottom Line
Planning is a spectrum:

No planning        ReAct                Plan-Execute         TOT
(just react)       (think + act)      (plan then act)      (explore + pick)
    Fast             Good               Structured           Expensive
    Dumb            Default              Complex tasks       Rare, niche 

For 80% of agents tasks, ReAct with dynamic replanning is the right default. Add upfrontend planning for complex, multi-phase tasks. Add ToT only when you genuinely need to explore multiple approaches.


