from __future__ import annotations

from enum import Enum
from typing import Awaitable, Literal, Protocol, TypeAlias, overload

from pydantic import BaseModel, Field, JsonValue
from typing_extensions import TypedDict

JsonPayload: TypeAlias = dict[str, JsonValue]

ToolName: TypeAlias = Literal[
    "account_summary",
    "positions",
    "recent_trades",
    "analytics_snapshot",
    "journal_entries",
    "playbook_setups",
    "notebook_context",
]


class EmptyToolArguments(TypedDict, total=False):
    pass


class LimitToolArguments(TypedDict):
    limit: int


ToolArguments: TypeAlias = EmptyToolArguments | LimitToolArguments


def _empty_tool_arguments() -> EmptyToolArguments:
    return {}


class AgentEventType(str, Enum):
    REQUEST_START = "request.start"
    REQUEST_CANCEL = "request.cancel"
    REQUEST_PING = "request.ping"
    RESPONSE_PONG = "response.pong"
    TOOL_CALL = "tool.call"
    TOOL_RESULT = "tool.result"
    RESPONSE_DELTA = "response.delta"
    RESPONSE_COMPLETED = "response.completed"
    RESPONSE_ERROR = "response.error"


class GraphEventType(str, Enum):
    MEMORY_RETRIEVED = "memory.retrieved"
    ROUTE_SELECTED = "route.selected"
    TOOL_STARTED = "tool.started"
    TOOL_COMPLETED = "tool.completed"


class AgentRoute(str, Enum):
    PORTFOLIO_ANALYST = "portfolio_analyst"
    JOURNAL_COACH = "journal_coach"
    TRADING_EDUCATOR = "trading_educator"


class MemoryKind(str, Enum):
    PREFERENCE = "preference"
    GOAL = "goal"
    PATTERN = "pattern"
    EVENT = "event"


class EventEmitter(Protocol):
    async def __call__(
        self,
        message_type: str,
        request_id: str,
        session_id: str,
        user_id: str,
        payload: JsonPayload,
    ) -> None:
        ...


class RequestStartPayload(BaseModel):
    model_config = {"populate_by_name": True}

    message: str


class ToolCallPayload(BaseModel):
    model_config = {"populate_by_name": True}

    tool_call_id: str = Field(alias="toolCallId")
    tool_name: ToolName = Field(alias="toolName")
    arguments: ToolArguments = Field(default_factory=_empty_tool_arguments)


class ToolResultPayload(BaseModel):
    model_config = {"populate_by_name": True}

    tool_call_id: str = Field(alias="toolCallId")
    tool_name: ToolName = Field(alias="toolName")
    ok: bool
    result: JsonPayload = Field(default_factory=dict)
    error: str | None = None


class ResponseDeltaPayload(BaseModel):
    text: str


class ResponseCompletedPayload(BaseModel):
    model_config = {"populate_by_name": True}

    text: str
    promoted_memory_uris: list[str] = Field(default_factory=list, alias="promotedMemoryUris")


class ResponseErrorPayload(BaseModel):
    message: str


class MemoryRetrievedItemPayload(BaseModel):
    model_config = {"populate_by_name": True}

    uri: str
    bucket: str
    kind: MemoryKind
    abstract: str
    score: float
    importance: float = 0.5
    confidence: float = 0.5
    age_days: float = Field(alias="ageDays")
    is_expired: bool = Field(alias="isExpired")


class MemoryRetrievedPayload(BaseModel):
    count: int
    items: list[MemoryRetrievedItemPayload]


class RouteSelectedPayload(BaseModel):
    route: AgentRoute


class ToolStartedPayload(BaseModel):
    model_config = {"populate_by_name": True}

    tool_name: ToolName = Field(alias="toolName")
    arguments: ToolArguments = Field(default_factory=_empty_tool_arguments)


class ToolCompletedPayload(BaseModel):
    model_config = {"populate_by_name": True}

    tool_name: ToolName = Field(alias="toolName")
    result: JsonPayload = Field(default_factory=dict)


class AgentEnvelope(BaseModel):
    type: AgentEventType
    request_id: str
    session_id: str
    user_id: str
    payload: JsonPayload = Field(default_factory=dict)

    model_config = {"extra": "ignore"}

    @classmethod
    def parse_wire(cls, raw: str) -> "AgentEnvelope":
        return cls.model_validate_json(raw)


@overload
def parse_payload_by_event(
    event: Literal[AgentEventType.REQUEST_START], payload: JsonPayload
) -> RequestStartPayload:
    ...


@overload
def parse_payload_by_event(
    event: Literal[AgentEventType.TOOL_RESULT], payload: JsonPayload
) -> ToolResultPayload:
    ...


def parse_payload_by_event(
    event: AgentEventType, payload: JsonPayload
) -> BaseModel:
    if event is AgentEventType.REQUEST_START:
        return RequestStartPayload.model_validate(payload)
    if event is AgentEventType.TOOL_RESULT:
        return ToolResultPayload.model_validate(payload)
    raise ValueError(f"Unsupported event type for typed payload parsing: {event.value}")


def validate_outbound_payload(message_type: AgentEventType | GraphEventType | str, payload: JsonPayload) -> JsonPayload:
    message_name = (
        message_type.value
        if isinstance(message_type, (AgentEventType, GraphEventType))
        else str(message_type)
    )
    if message_name == AgentEventType.RESPONSE_DELTA.value:
        ResponseDeltaPayload.model_validate(payload)
    elif message_name == AgentEventType.RESPONSE_COMPLETED.value:
        ResponseCompletedPayload.model_validate(payload)
    elif message_name == AgentEventType.RESPONSE_ERROR.value:
        ResponseErrorPayload.model_validate(payload)
    elif message_name == AgentEventType.TOOL_CALL.value:
        ToolCallPayload.model_validate(payload)
    elif message_name == GraphEventType.MEMORY_RETRIEVED.value:
        MemoryRetrievedPayload.model_validate(payload)
    elif message_name == GraphEventType.ROUTE_SELECTED.value:
        RouteSelectedPayload.model_validate(payload)
    elif message_name == GraphEventType.TOOL_STARTED.value:
        ToolStartedPayload.model_validate(payload)
    elif message_name == GraphEventType.TOOL_COMPLETED.value:
        ToolCompletedPayload.model_validate(payload)
    else:
        # Unknown/custom internal events intentionally pass-through.
        return payload
    return payload


__all__ = [
    "AgentEventType",
    "GraphEventType",
    "AgentRoute",
    "MemoryKind",
    "JsonValue",
    "JsonPayload",
    "ToolName",
    "EmptyToolArguments",
    "LimitToolArguments",
    "ToolArguments",
    "EventEmitter",
    "RequestStartPayload",
    "ToolCallPayload",
    "ToolResultPayload",
    "ResponseDeltaPayload",
    "ResponseCompletedPayload",
    "ResponseErrorPayload",
    "MemoryRetrievedPayload",
    "MemoryRetrievedItemPayload",
    "RouteSelectedPayload",
    "ToolStartedPayload",
    "ToolCompletedPayload",
    "AgentEnvelope",
    "parse_payload_by_event",
    "validate_outbound_payload",
]
