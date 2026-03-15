# An agent is an LLM in a loop -> given a goal, it decides what actions to take, executes
them, observes results, and continues until the goal is done (or it fails)


# The core loop 
Goal -> Think -> Act -> Observe -> Think -> Act -> ... > Done 
# (Reasoning + Acting) approach

# Core components 
1. The LLM (the brain) Model decides what to do next. It reads the current state of the
world and picks an action. Everything else serves this.

2. Tools Functions the agent can call - web search, code execution, file I/O, API calls, 
database queries, sending emails. The agent is only as powerful as its toolset.

3. Memory 
   * In-contet -> the conversation the model is currently reading 
   * External -> Vector DBs, SQL, files. Let the agent remember things across sessions
   and retrieve relevant info from huge corpora (huge content like documnets)

4. Planning How the agent breaks down complex tasks. Strategies:
   * Chain of Thought - think step by step inline 
   * ReAct - Interleave reasoning and tool use 
   * Tree of Thoughts - explore multiple reasoning branches 
   * Plan and Execute - make a full plan first, then execute each step 

5. Orchestration The runtime that manages the loop: routing the LLM output, parsing tool
calls, executing them, feeding results back in. Frameworks like LangGraph, LlamaIndex, or 
raw API loops handle this

# Permissions 
The goal here is for the agent should only be able to do what the user has explicitly permitted. Breaks down into:

* Scope -> What tools/APIs does this agent have access to? A coding agent shouldn't be able to send emails

* Confirmation gates -> for irreversible or high-stakes actions(deleting files, sending messages, spending money), pause and ask the user before executing

* Least prvilege -> Give the agent the minimum permissions it needs. Don't give it admin creds if read-only works 

* Audit trail -> log every action the agent takes so you can debug and the user can review


# Agent Architectures 

* Signle agent -> one LLM with tools in a loop. Good for focused tasks

* Multi-agent -> multiple specialized agents coordinated by an orchestrator. Example: a "manager" agent that delegates to a "research" agent and a "coder" agent. Better for complex tasks that need specialization

* Humian-in-the-loop -> agents pauses at key decision points for human approval. Very important

# Problems 

* Reliability -> LLMs make mistakes. Agents compound mistakes across multiple steps. Ome bad tool call can derail the whole task. Mitigation: good evals, retyr logic, himan checkpoints or (break the steps into smaller steps, then use that has checkpoints)

* Context limits -> long tasks fill up the context window. I need stratgies for summarization, memory offloading, or chunking tasks.

* Tool design -> poorly designed tools (bad descriptions, ambiguous parameters) cause the agent to misuse them. The quality the tool descriptions really matters. 

* Prompt injection -> if the agent reads external content (web pages, docs, emails), malicious content can try to hijack its instructions. TODO: Look at measures to prevent this

* Infinite loops -> agents get stuck trying the same failing action repeatedly. I'll need max step limits and loop detection here 

* Cost -> agents make many LLM calls. A task that "feels simple" might cost 50 calls.

