from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Protocol, TypeAlias, cast

from langgraph.graph import END, StateGraph
from typing_extensions import NotRequired, TypedDict

from tradstry_agents.memory import OpenVikingMemoryStore
from tradstry_agents.prompts import PromptLibrary
from tradstry_agents.providers import ChatProvider
from tradstry_agents.schemas import (
    AgentRoute,
    EmptyToolArguments,
    GraphEventType,
    JsonPayload,
    LimitToolArguments,
    MemoryKind,
    MemoryRetrievedPayload,
    RouteSelectedPayload,
    EventEmitter,
    ToolName,
)
from tradstry_agents.tools import ToolRuntime


class PortfolioAnalystToolResults(TypedDict):
    accountSummary: JsonPayload
    positions: JsonPayload
    recentTrades: JsonPayload
    analyticsSnapshot: NotRequired[JsonPayload]


class JournalCoachToolResults(TypedDict):
    journalEntries: JsonPayload
    playbookSetups: JsonPayload
    notebookContext: JsonPayload
    analyticsSnapshot: NotRequired[JsonPayload]


class TradingEducatorToolResults(TypedDict):
    generalKnowledge: str


class PendingToolResults(TypedDict, total=False):
    pass


ToolResults: TypeAlias = (
    PendingToolResults
    | PortfolioAnalystToolResults
    | JournalCoachToolResults
    | TradingEducatorToolResults
)


class RetrievedMemoryForPrompt(TypedDict):
    uri: str
    bucket: str
    kind: MemoryKind
    abstract: str
    content: str
    score: float
    importance: float
    confidence: float
    age_days: float
    is_expired: bool


class AgentState(TypedDict):
    request_id: str
    session_id: str
    user_id: str
    message: str
    route: AgentRoute
    retrieved_memory: list[RetrievedMemoryForPrompt]
    tool_results: ToolResults
    used_memory_uris: list[str]
    used_tools: list[ToolName]
    final_answer: str


@dataclass(frozen=True)
class AgentRunResult:
    final_answer: str
    route: AgentRoute
    used_memory_uris: list[str]
    used_tools: list[ToolName]


class CompiledAgentGraph(Protocol):
    async def ainvoke(self, state: AgentState) -> AgentState:
        ...


class AgentGraphRunner:
    def __init__(
        self,
        *,
        chat_provider: ChatProvider,
        memory_store: OpenVikingMemoryStore,
        prompt_library: PromptLibrary,
        tool_runtime: ToolRuntime,
        emit: EventEmitter,
    ):
        self._chat_provider = chat_provider
        self._memory_store = memory_store
        self._prompt_library = prompt_library
        self._tool_runtime = tool_runtime
        self._emit = emit
        self._graph: CompiledAgentGraph = self._build_graph()

    async def run(
        self, *, request_id: str, session_id: str, user_id: str, message: str
    ) -> AgentRunResult:
        if not message.strip():
            raise ValueError("message must be non-empty")
        initial_state: AgentState = {
            "request_id": request_id,
            "session_id": session_id,
            "user_id": user_id,
            "message": message,
            "route": AgentRoute.TRADING_EDUCATOR,
            "retrieved_memory": [],
            "tool_results": _pending_tool_results(),
            "used_memory_uris": [],
            "used_tools": [],
            "final_answer": "",
        }
        result = await self._graph.ainvoke(initial_state)
        return AgentRunResult(
            final_answer=result["final_answer"],
            route=result["route"],
            used_memory_uris=result["used_memory_uris"],
            used_tools=result["used_tools"],
        )

    def _build_graph(self) -> CompiledAgentGraph:
        graph = StateGraph(AgentState)
        graph.add_node("load_memory", self._load_memory)
        graph.add_node("supervisor", self._supervisor)
        graph.add_node("portfolio_analyst", self._portfolio_analyst)
        graph.add_node("journal_coach", self._journal_coach)
        graph.add_node("trading_educator", self._trading_educator)
        graph.add_node("compose", self._compose)
        graph.set_entry_point("load_memory")
        graph.add_edge("load_memory", "supervisor")
        graph.add_conditional_edges(
            "supervisor",
            lambda state: state["route"],
            {
                AgentRoute.PORTFOLIO_ANALYST: "portfolio_analyst",
                AgentRoute.JOURNAL_COACH: "journal_coach",
                AgentRoute.TRADING_EDUCATOR: "trading_educator",
            },
        )
        graph.add_edge("portfolio_analyst", "compose")
        graph.add_edge("journal_coach", "compose")
        graph.add_edge("trading_educator", "compose")
        graph.add_edge("compose", END)
        return cast(CompiledAgentGraph, graph.compile())

    async def _load_memory(self, state: AgentState) -> AgentState:
        retrieved = await self._memory_store.retrieve_context(
            user_id=state["user_id"],
            session_id=state["session_id"],
            query=state["message"],
        )
        memory_items: list[RetrievedMemoryForPrompt] = []
        for item in retrieved:
            payload: RetrievedMemoryForPrompt = {
                "uri": item.uri,
                "bucket": item.bucket,
                "kind": item.kind,
                "abstract": item.abstract,
                "content": item.content,
                "score": item.score,
                "importance": item.importance,
                "confidence": item.confidence,
                "age_days": item.age_days,
                "is_expired": item.is_expired,
            }
            memory_items.append(payload)

        await self._emit(
            GraphEventType.MEMORY_RETRIEVED.value,
            state["request_id"],
            state["session_id"],
            state["user_id"],
            MemoryRetrievedPayload(
                count=len(memory_items),
                items=[
                    {
                        "uri": item["uri"],
                        "bucket": item["bucket"],
                        "kind": item["kind"],
                        "abstract": item["abstract"],
                        "score": item["score"],
                        "importance": item["importance"],
                        "confidence": item["confidence"],
                        "ageDays": item["age_days"],
                        "isExpired": item["is_expired"],
                    }
                    for item in memory_items
                ],
            ).model_dump(),
        )

        return {
            **state,
            "retrieved_memory": memory_items,
            "used_memory_uris": [item["uri"] for item in memory_items],
        }

    async def _supervisor(self, state: AgentState) -> AgentState:
        message = state["message"].lower()
        route = self._pick_route(message)
        await self._emit(
            GraphEventType.ROUTE_SELECTED.value,
            state["request_id"],
            state["session_id"],
            state["user_id"],
            RouteSelectedPayload(route=route).model_dump(),
        )
        return {
            **state,
            "route": route,
        }

    async def _portfolio_analyst(self, state: AgentState) -> AgentState:
        message = state["message"].lower()
        results, used_tools = await self._portfolio_analyst_results(message)
        return {
            **state,
            "tool_results": results,
            "used_tools": used_tools,
        }

    async def _journal_coach(self, state: AgentState) -> AgentState:
        message = state["message"].lower()
        results, used_tools = await self._journal_coach_results(message)
        return {
            **state,
            "tool_results": results,
            "used_tools": used_tools,
        }

    async def _trading_educator(self, state: AgentState) -> AgentState:
        return {
            **state,
            "tool_results": self._trading_educator_results(),
            "used_tools": [],
        }

    async def _compose(self, state: AgentState) -> AgentState:
        system_prompt = self._prompt_library.compose_system_prompt(state["route"])
        memory = self._prompt_library.format_memory_context(state["retrieved_memory"])
        tool_results = json.dumps(state["tool_results"], indent=2)
        user_prompt = "\n\n".join(
            [
                f"Route: {state['route'].value}",
                f"User message: {state['message']}",
                "Retrieved memory:",
                memory,
                "Tool results:",
                tool_results,
            ]
        )
        final_answer = await self._chat_provider.complete(
            system_prompt=system_prompt, user_prompt=user_prompt
        )
        return {
            **state,
            "final_answer": final_answer,
        }

    @staticmethod
    def _pick_route(message: str) -> AgentRoute:
        if any(token in message for token in ("journal", "playbook", "mistake", "setup", "note")):
            return AgentRoute.JOURNAL_COACH
        if any(
            token in message
            for token in ("account", "portfolio", "position", "trade", "pnl", "win rate")
        ):
            return AgentRoute.PORTFOLIO_ANALYST
        return AgentRoute.TRADING_EDUCATOR

    async def _portfolio_analyst_results(
        self, message: str
    ) -> tuple[PortfolioAnalystToolResults, list[ToolName]]:
        used_tools: list[ToolName] = ["account_summary", "positions", "recent_trades"]
        results: PortfolioAnalystToolResults = {
            "accountSummary": await self._tool_runtime.call(
                "account_summary", _empty_tool_arguments()
            ),
            "positions": await self._tool_runtime.call("positions", _empty_tool_arguments()),
            "recentTrades": await self._tool_runtime.call(
                "recent_trades", _limit_tool_arguments(8)
            ),
        }
        if any(token in message for token in ("performance", "analytics", "win rate", "calendar")):
            results["analyticsSnapshot"] = await self._tool_runtime.call(
                "analytics_snapshot", _empty_tool_arguments()
            )
            used_tools.append("analytics_snapshot")
        return (results, used_tools)

    async def _journal_coach_results(
        self, message: str
    ) -> tuple[JournalCoachToolResults, list[ToolName]]:
        used_tools: list[ToolName] = ["journal_entries", "playbook_setups", "notebook_context"]
        results: JournalCoachToolResults = {
            "journalEntries": await self._tool_runtime.call(
                "journal_entries", _limit_tool_arguments(8)
            ),
            "playbookSetups": await self._tool_runtime.call(
                "playbook_setups", _empty_tool_arguments()
            ),
            "notebookContext": await self._tool_runtime.call(
                "notebook_context", _limit_tool_arguments(8)
            ),
        }
        if "analytics" in message or "win rate" in message:
            results["analyticsSnapshot"] = await self._tool_runtime.call(
                "analytics_snapshot", _empty_tool_arguments()
            )
            used_tools.append("analytics_snapshot")
        return (results, used_tools)

    @staticmethod
    def _trading_educator_results() -> TradingEducatorToolResults:
        return {"generalKnowledge": "No live account tools were required."}


def _empty_tool_arguments() -> EmptyToolArguments:
    return {}


def _limit_tool_arguments(limit: int) -> LimitToolArguments:
    return {"limit": limit}


def _pending_tool_results() -> PendingToolResults:
    return {}
