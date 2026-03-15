## Memory in Agents 

# Types
1. In-Context memory
2. Working/short-term memory
3. long-term memory
4. Semantic/knowledge memory 

## In-Context memory 
The model's context window -> everything it can see right now. Very fast, zero latency

* Limited -> Top models have less than 300k tokens 
* Ephemeral -> gone when the conversation ends 
* Expensive -> every token in context costs money on every call 

# What should live here:
* The system prompt (agent's identity, tools, rules)
* The conversation history
* Tool call results 

## Working memory 
This is state you explicity manage outside the context window but keep close -> structured data, the orchestration layer tracks and selectively injects into context
"The agent's clipboard'

# Examples
* Current task + subtasks 
* Intermediate results 
* A running plan 
* Entities extracted so far (people, dates URLs found)

I store this as structured data (JSON) in the application layer, and i decide when and how much of it to inject into the prompt 

```py
state = {
    "goal": "Research and summarize AI papers from 2024",
    "subtasks": ["search arxiv", "filter relevant", "summarize each"],
    "current_step": 1,
    "papers_found": [...],
    "summaries": []
}
```

This lets me keep the context widnow clean while the agent maintains awareness of where it is in a long task. 


## long-term (Episodic) Memory (Past experiences)

This is whay the agent remembers from previous sessions -> past conversations, past tasks, past mistakes.

Without this, every session starts fresh. The agent has no idea it already tried to do this task last Tuesday and hit a rate limit, or that this user prefers concise answers

# How it works
I store past interactions in a database. When a new session starts, i retrieve relevant memories and inject them into the context. 

[New task comes in]
-> Embed the task
-> Query memory store: "what's relvate to this?"
-> Retrieve top-k memories 
-> Inject into system prompt or context
-> Agent now "remembers" relevant past experiences 

# What to store
- Full conversation summaries (not raw transcripts - too long)
- key facts learned about the user 
- Past task outcomes (succeeded, failed, why)
- User feedback and corrections 

# Storage options 
- Vetor database -> for semantic search
- SQL database -> for structured facts, user profiles 


## Semantic memory
This is a domain knowledge the agent can look up - not personal history, but facts about the world or the specific domain

# Examples 
- Your company's internal docs
- A codebase
- A product catalog
- Medical literature 
- Legal documents 

This is the RAG (Retrieval-Augmented Generation) layer. The agent queries it like a knowledge base 

* The difference from episodic memory: episodic is "what happened before", sematic is "what is true about the domain"


## Retrieval problem 
All external memory (episodic + semantic) lives outside context, so i need to retrieve the right stuff at the right time 

# Strategies 

* Dense retrieval (vector search)
Embed the query, find semantically similar chunks. Great for fuzzy/conceptual search

query: "how do i handle auth errors?"
-> finds: chunks about OAuth, token expiry, 401 handling 

* Sparse retrieval (Keyword/BM25)
Classic keyword matching. Great for exact terms, IDs, proper nouns that embeddings miss.

* Hybrid search
Combine both. This is what i should use in production.
Dense catches meaning, sparse catches exact matches 

* Re-ranking
After retrieval, use a cross-encoder model to re-score results for relevance. Cost more but dramatically improves quality

* Structured retrieval
Someimes i don't need embeddings - i need a SQL query. "Find all tasks this user completed last month" is a database query, not a vector search


## Memory writing 

# Strategies 

* Summarization before storage 
Don't store raw conversations. Run them through the LLM first. "Summarize the key facts, decisions, and outcomes from this conversation in 3-5 bulltet points"

* Entity extraction
Pull out structured facts explicitly

- Example 
User name: Alex
User preference: prefers Python over JS
Project: building a RAG pipeline 
Last session outcome: got stuck on chunking strategy 

* Conflict resolution 
What if new information contradicts stored memory? I need a logic to update or flag conflicting facts

* Forgetting 
not everything should be kept forever. Implement TTL (time-to-live) on memories, or decay scores over time. Stale memories can mislead the agent


## Context Management Strategies 
When the context fills up, several options:

* Summarization
Periodically compress old conersation history

```
[turn 1-20 summary]: "Agents researched 5 papers, found 3 relevant, hit rate limit on arxiv, switched to sematic scholar"
[turn 21+]: full detail
```

* Sliding window 
Only keep the last N turns in full details. Drop older ones or replace with summary

* Selective injection
Don't dump all memory into context. Only injects what's relevant to the vurrent step. This requires good retrieval

* Memory hierarcy
Thinking of it like CPU cache:

L1 (fastest, smallest): current reasoning in-context
L2 (fast): working state injected into prompt
L3 (slower): retrieved episodic/semantic memories 
L4 (slowest): raw database, searched on demand 
