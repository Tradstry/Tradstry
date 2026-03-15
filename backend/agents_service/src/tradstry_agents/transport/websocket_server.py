from __future__ import annotations

import asyncio
import contextlib
import json
import logging
import os
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from pathlib import Path
from uuid import uuid4

import uvicorn
from fastapi import FastAPI, WebSocket, WebSocketDisconnect

from tradstry_agents.config import load_settings
from tradstry_agents.graphs import AgentGraphRunner
from tradstry_agents.memory import OpenVikingMemoryStore
from tradstry_agents.providers import GroqChatProvider
from tradstry_agents.prompts import PromptLibrary
from tradstry_agents.schemas import (
    AgentEnvelope,
    AgentEventType,
    GraphEventType,
    RequestStartPayload,
    JsonPayload,
    ResponseCompletedPayload,
    ResponseDeltaPayload,
    ResponseErrorPayload,
    ToolArguments,
    ToolCallPayload,
    ToolName,
    ToolResultPayload,
    parse_payload_by_event,
    validate_outbound_payload,
)
from tradstry_agents.tools import ToolContext, ToolRuntime

logger = logging.getLogger(__name__)


def _configure_application_logging() -> None:
    app_logger = logging.getLogger("tradstry_agents")
    app_logger.setLevel(logging.INFO)

    if app_logger.handlers:
        return

    handler = logging.StreamHandler()
    handler.setLevel(logging.INFO)
    handler.setFormatter(
        logging.Formatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s")
    )
    app_logger.addHandler(handler)


class AgentServiceRuntime:
    def __init__(self) -> None:
        self.settings = load_settings()
        self.chat_provider = GroqChatProvider(self.settings)
        self.memory_store = OpenVikingMemoryStore(self.settings)
        self.prompt_library = PromptLibrary(self.settings)

    async def startup(self) -> None:
        await self.memory_store.initialize()

    async def shutdown(self) -> None:
        await self.memory_store.close()


class AgentConnection:
    def __init__(self, websocket: WebSocket, runtime: AgentServiceRuntime):
        self.websocket = websocket
        self.runtime = runtime
        self._request_tasks: dict[str, asyncio.Task[None]] = {}
        self._pending_tool_calls: dict[str, asyncio.Future[JsonPayload]] = {}

    async def handle(self) -> None:
        await self.websocket.accept()
        client = getattr(self.websocket, "client", None)
        logger.info("agents websocket accepted client=%s", client)
        try:
            while True:
                raw = await self.websocket.receive_text()
                envelope = AgentEnvelope.parse_wire(raw)
                logger.info(
                    "received envelope type=%s request_id=%s session_id=%s user_id=%s",
                    envelope.type.value,
                    envelope.request_id,
                    envelope.session_id,
                    envelope.user_id,
                )
                await self._dispatch(envelope)
        except WebSocketDisconnect:
            logger.info("agents websocket disconnected")
            await self._cancel_all()

    async def _dispatch(self, envelope: AgentEnvelope) -> None:
        if envelope.type is AgentEventType.REQUEST_START:
            logger.info(
                "dispatching request.start request_id=%s session_id=%s",
                envelope.request_id,
                envelope.session_id,
            )
            request_task = asyncio.create_task(self._run_request(envelope))
            self._request_tasks[envelope.request_id] = request_task
        elif envelope.type is AgentEventType.REQUEST_CANCEL:
            logger.info(
                "dispatching request.cancel request_id=%s session_id=%s",
                envelope.request_id,
                envelope.session_id,
            )
            cancelled_task = self._request_tasks.get(envelope.request_id)
            if cancelled_task is not None:
                self._request_tasks.pop(envelope.request_id, None)
                cancelled_task.cancel()
        elif envelope.type is AgentEventType.REQUEST_PING:
            await self._send(
                AgentEventType.RESPONSE_PONG.value,
                envelope.request_id,
                envelope.session_id,
                envelope.user_id,
                {},
            )
        elif envelope.type is AgentEventType.TOOL_RESULT:
            tool_result = parse_payload_by_event(AgentEventType.TOOL_RESULT, envelope.payload)
            tool_call_id = tool_result.tool_call_id
            logger.info(
                "received tool.result request_id=%s tool_call_id=%s tool_name=%s ok=%s",
                envelope.request_id,
                tool_call_id,
                tool_result.tool_name,
                tool_result.ok,
            )
            future = self._pending_tool_calls.pop(tool_call_id, None)
            if future is not None and not future.done():
                future.set_result(tool_result.model_dump(by_alias=True))

    async def _run_request(self, envelope: AgentEnvelope) -> None:
        try:
            message_payload = parse_payload_by_event(
                AgentEventType.REQUEST_START, envelope.payload
            )
            if not isinstance(message_payload, RequestStartPayload):
                raise ValueError("request.start payload is invalid")

            message = message_payload.message.strip()
            if not message:
                raise ValueError("request.start requires payload.message")

            logger.info(
                "request started request_id=%s session_id=%s user_id=%s message_len=%s",
                envelope.request_id,
                envelope.session_id,
                envelope.user_id,
                len(message),
            )

            await self.runtime.memory_store.append_user_turn(
                user_id=envelope.user_id,
                session_id=envelope.session_id,
                content=message,
            )

            tool_runtime = ToolRuntime(
                context=ToolContext(
                    request_id=envelope.request_id,
                    session_id=envelope.session_id,
                    user_id=envelope.user_id,
                ),
                invoker=lambda tool_name, arguments: self._call_tool(
                    envelope=envelope, tool_name=tool_name, arguments=arguments
                ),
                emit=self._send,
                timeout_seconds=self.runtime.settings.tool_timeout_seconds,
            )
            graph_runner = AgentGraphRunner(
                chat_provider=self.runtime.chat_provider,
                memory_store=self.runtime.memory_store,
                prompt_library=self.runtime.prompt_library,
                tool_runtime=tool_runtime,
                emit=self._send,
            )
            run_result = await graph_runner.run(
                request_id=envelope.request_id,
                session_id=envelope.session_id,
                user_id=envelope.user_id,
                message=message,
            )
            final_answer = run_result.final_answer
            logger.info(
                "graph completed request_id=%s route=%s answer_len=%s used_memory=%s used_tools=%s",
                envelope.request_id,
                run_result.route.value,
                len(final_answer),
                len(run_result.used_memory_uris),
                len(run_result.used_tools),
            )

            await self.runtime.memory_store.append_assistant_turn(
                user_id=envelope.user_id,
                session_id=envelope.session_id,
                content=final_answer,
            )
            promoted = await self.runtime.memory_store.finalize_turn(
                user_id=envelope.user_id,
                session_id=envelope.session_id,
                request_text=message,
                response_text=final_answer,
                route=run_result.route,
                used_memory_uris=run_result.used_memory_uris,
                used_tools=run_result.used_tools,
            )
            logger.info(
                "request finalized request_id=%s session_id=%s promoted_memories=%s",
                envelope.request_id,
                envelope.session_id,
                len(promoted),
            )
            await self._send(
                AgentEventType.RESPONSE_COMPLETED.value,
                envelope.request_id,
                envelope.session_id,
                envelope.user_id,
                ResponseCompletedPayload(
                    text=final_answer,
                    promoted_memory_uris=promoted,
                ).model_dump(by_alias=True),
            )
        except asyncio.CancelledError:
            logger.warning(
                "request cancelled request_id=%s session_id=%s",
                envelope.request_id,
                envelope.session_id,
            )
            await self._send(
                AgentEventType.RESPONSE_ERROR.value,
                envelope.request_id,
                envelope.session_id,
                envelope.user_id,
                ResponseErrorPayload(message="request cancelled").model_dump(),
            )
            raise
        except Exception as exc:
            logger.exception(
                "request failed request_id=%s session_id=%s error=%s",
                envelope.request_id,
                envelope.session_id,
                exc,
            )
            await self._send(
                AgentEventType.RESPONSE_ERROR.value,
                envelope.request_id,
                envelope.session_id,
                envelope.user_id,
                ResponseErrorPayload(message=str(exc)).model_dump(),
            )
        finally:
            self._request_tasks.pop(envelope.request_id, None)

    async def _call_tool(
        self, *, envelope: AgentEnvelope, tool_name: ToolName, arguments: ToolArguments
    ) -> JsonPayload:
        tool_call_id = uuid4().hex
        future: asyncio.Future[JsonPayload] = asyncio.get_running_loop().create_future()
        self._pending_tool_calls[tool_call_id] = future
        logger.info(
            "sending tool.call request_id=%s tool_call_id=%s tool_name=%s",
            envelope.request_id,
            tool_call_id,
            tool_name,
        )
        await self._send(
            AgentEventType.TOOL_CALL.value,
            envelope.request_id,
            envelope.session_id,
            envelope.user_id,
            ToolCallPayload(
                tool_call_id=tool_call_id,
                tool_name=tool_name,
                arguments=arguments,
            ).model_dump(by_alias=True),
        )
        tool_result_raw = await future
        tool_result = ToolResultPayload.model_validate(tool_result_raw)
        logger.info(
            "tool.call completed request_id=%s tool_call_id=%s tool_name=%s ok=%s",
            envelope.request_id,
            tool_call_id,
            tool_name,
            tool_result.ok,
        )
        if not tool_result.ok:
            raise RuntimeError(tool_result.error or f"{tool_name} failed")
        return dict(tool_result.result)

    async def _send(
        self,
        message_type: str,
        request_id: str,
        session_id: str,
        user_id: str,
        payload: JsonPayload,
    ) -> None:
        if message_type == AgentEventType.RESPONSE_DELTA.value:
            validate_outbound_payload(AgentEventType.RESPONSE_DELTA, payload)
        elif message_type == AgentEventType.RESPONSE_COMPLETED.value:
            validate_outbound_payload(AgentEventType.RESPONSE_COMPLETED, payload)
        elif message_type == AgentEventType.RESPONSE_ERROR.value:
            validate_outbound_payload(AgentEventType.RESPONSE_ERROR, payload)
        elif message_type == AgentEventType.TOOL_CALL.value:
            validate_outbound_payload(AgentEventType.TOOL_CALL, payload)
        elif message_type == GraphEventType.MEMORY_RETRIEVED.value:
            validate_outbound_payload(GraphEventType.MEMORY_RETRIEVED, payload)
        elif message_type == GraphEventType.ROUTE_SELECTED.value:
            validate_outbound_payload(GraphEventType.ROUTE_SELECTED, payload)
        elif message_type == GraphEventType.TOOL_STARTED.value:
            validate_outbound_payload(GraphEventType.TOOL_STARTED, payload)
        elif message_type == GraphEventType.TOOL_COMPLETED.value:
            validate_outbound_payload(GraphEventType.TOOL_COMPLETED, payload)

        if message_type != AgentEventType.RESPONSE_DELTA.value:
            logger.info(
                "sending envelope type=%s request_id=%s session_id=%s",
                message_type,
                request_id,
                session_id,
            )

        envelope = {
            "type": message_type,
            "request_id": request_id,
            "session_id": session_id,
            "user_id": user_id,
            "payload": payload,
        }
        await self.websocket.send_text(json.dumps(envelope))

    async def _cancel_all(self) -> None:
        for task in list(self._request_tasks.values()):
            task.cancel()
        for task in list(self._request_tasks.values()):
            with contextlib.suppress(Exception):
                await task
        for future in list(self._pending_tool_calls.values()):
            future.cancel()
        self._pending_tool_calls.clear()
        self._request_tasks.clear()


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncIterator[None]:
    _configure_application_logging()
    runtime = AgentServiceRuntime()
    logger.info("agents service startup begin")
    await runtime.startup()
    app.state.runtime = runtime
    try:
        yield
    finally:
        logger.info("agents service shutdown begin")
        await runtime.shutdown()


def create_app() -> FastAPI:
    settings = load_settings()
    app = FastAPI(lifespan=lifespan)

    @app.get("/healthz")
    async def healthz() -> dict[str, str]:
        return {"status": "ok"}

    @app.websocket(settings.websocket_path)
    async def agent_websocket(websocket: WebSocket) -> None:
        connection = AgentConnection(websocket, websocket.app.state.runtime)
        await connection.handle()

    return app


def run() -> None:
    settings = load_settings()
    reload_enabled = os.getenv("AGENTS_SERVICE_RELOAD", "false").lower() in {
        "1",
        "true",
        "yes",
        "on",
    }
    project_src_dir = str(Path(__file__).resolve().parents[2])
    uvicorn.run(
        "main:app",
        host=settings.host,
        port=settings.port,
        reload=reload_enabled,
        reload_dirs=[project_src_dir] if reload_enabled else None,
        log_level="info",
    )
