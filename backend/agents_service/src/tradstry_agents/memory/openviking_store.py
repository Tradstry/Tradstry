from __future__ import annotations

import contextlib
import inspect
import json
import logging
import os
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Protocol

from openviking import AsyncOpenViking  # type: ignore[import-untyped]
from openviking.message.part import TextPart  # type: ignore[import-untyped]

from tradstry_agents.config import Settings
from tradstry_agents.schemas import AgentRoute, MemoryKind, ToolName

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class RetrievedMemory:
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


@dataclass(frozen=True)
class SessionTurn:
    role: str
    content: str
    created_at: str | None


class OpenVikingSession(Protocol):
    def add_message(self, role: str, /, *args: object, **kwargs: object) -> object:
        ...

    def load(self) -> object:
        ...

    def commit(self) -> object:
        ...

    @property
    def messages(self) -> object:
        ...


class OpenVikingClient(Protocol):
    def session(self, *, session_id: str) -> OpenVikingSession:
        ...

    async def initialize(self) -> None:
        ...

    async def close(self) -> None:
        ...

    async def search(
        self,
        *,
        query: str,
        target_uri: str = "",
        session_id: str | None = None,
        limit: int = 10,
    ) -> object:
        ...

    async def abstract(self, uri: str) -> str:
        ...

    async def overview(self, uri: str) -> str:
        ...

    async def read(self, uri: str) -> str:
        ...


class OpenVikingMemoryStore:
    def __init__(self, settings: Settings):
        self._settings = settings
        self._client: OpenVikingClient | None = None
        self._base = settings.openviking_data_root
        # Single session object reused across the entire turn lifecycle.
        # Keyed by scoped session ID (user_id:session_id).
        self._active_sessions: dict[str, OpenVikingSession] = {}

    async def initialize(self) -> None:
        self._base.mkdir(parents=True, exist_ok=True)
        for bucket in ("session", "user", "agent", "queue", "index"):
            (self._base / bucket).mkdir(parents=True, exist_ok=True)
        if self._settings.openviking_client_enabled and self._has_openviking_embedding_config():
            embedding_config = self._openviking_embedding_config()
            vlm_config = self._openviking_vlm_config()
            logger.info(
                "memory store initializing openviking client base=%s provider=%s model=%s dimension=%s vlm_provider=%s vlm_model=%s",
                self._base,
                embedding_config.get("provider") if embedding_config else "none",
                embedding_config.get("model") if embedding_config else "none",
                embedding_config.get("dimension") if embedding_config else "none",
                vlm_config.get("provider") if vlm_config else "none",
                vlm_config.get("model") if vlm_config else "none",
            )
            self._ensure_openviking_config()
            self._client = AsyncOpenViking(path=str(self._base))
            await self._client.initialize()
            logger.info(
                "openviking runtime ready base=%s provider=%s model=%s dimension=%s vlm_provider=%s vlm_model=%s queue_enabled=true semantic_pipeline_enabled=true",
                self._base,
                embedding_config.get("provider") if embedding_config else "none",
                embedding_config.get("model") if embedding_config else "none",
                embedding_config.get("dimension") if embedding_config else "none",
                vlm_config.get("provider") if vlm_config else "none",
                vlm_config.get("model") if vlm_config else "none",
            )
            return
        logger.info(
            "memory store running without openviking client base=%s openviking_client_enabled=%s",
            self._base,
            self._settings.openviking_client_enabled,
        )
        logger.warning(
            "OpenViking is unavailable or disabled. Session history retrieval, session persistence, and promoted memories will be dropped."
        )

    async def close(self) -> None:
        if self._client is not None:
            logger.info("memory store closing openviking client")
            await self._client.close()
            self._client = None
        self._active_sessions.clear()

    async def append_user_turn(self, *, user_id: str, session_id: str, content: str) -> None:
        logger.info(
            "append_user_turn user_id=%s session_id=%s content_len=%s",
            user_id,
            session_id,
            len(content),
        )
        if self._client is not None:
            logger.info(
                "append_user_turn session_backend=openviking user_id=%s session_id=%s",
                user_id,
                session_id,
            )
            await self._append_session_message(
                self._get_or_create_session(session_id, user_id), "user", content
            )
            return
        logger.info(
            "append_user_turn skipped_openviking_unavailable user_id=%s session_id=%s",
            user_id,
            session_id,
        )
        logger.warning(
            "Dropping user turn because OpenViking is unavailable session_id=%s user_id=%s",
            session_id,
            user_id,
        )

    async def append_assistant_turn(self, *, user_id: str, session_id: str, content: str) -> None:
        logger.info(
            "append_assistant_turn user_id=%s session_id=%s content_len=%s",
            user_id,
            session_id,
            len(content),
        )
        if self._client is not None:
            logger.info(
                "append_assistant_turn session_backend=openviking user_id=%s session_id=%s",
                user_id,
                session_id,
            )
            await self._append_session_message(
                self._get_or_create_session(session_id, user_id), "assistant", content
            )
            return
        logger.info(
            "append_assistant_turn skipped_openviking_unavailable user_id=%s session_id=%s",
            user_id,
            session_id,
        )
        logger.warning(
            "Dropping assistant turn because OpenViking is unavailable session_id=%s user_id=%s",
            session_id,
            user_id,
        )

    async def retrieve_context(
        self, *, user_id: str, session_id: str, query: str, limit: int | None = None
    ) -> list[RetrievedMemory]:
        logger.info(
            "retrieve_context user_id=%s session_id=%s query_len=%s limit=%s",
            user_id,
            session_id,
            len(query),
            limit,
        )
        if self._client is None:
            logger.info("retrieve_context openviking_client_unavailable user_id=%s", user_id)
            return []

        retrieval_limit = limit or self._settings.memory_max_retrieval_count
        scoped_id = _scoped_session_id(user_id, session_id)

        # OpenViking operates under the_default_user() scope internally.
        # target_uri is intentionally omitted — the original viking://user/{user_id}
        # URI was never written to under the default scope, causing all retrievals
        # to return empty. User isolation is handled via scoped session IDs instead.
        logger.info(
            "retrieve_context native_openviking_search user_id=%s session_id=%s limit=%s",
            user_id,
            session_id,
            retrieval_limit,
        )
        try:
            raw_result = await self._client.search(
                query=query,
                target_uri="viking://user/default",
                session_id=scoped_id,
                limit=retrieval_limit,
            )
        except Exception as exc:
            logger.warning(
                "retrieve_context native_search_failed user_id=%s session_id=%s error=%s",
                user_id,
                session_id,
                exc,
            )
            return []

        ranked = await self._coerce_openviking_search_results(raw_result)
        logger.info(
            "retrieve_context native_search_completed user_id=%s session_id=%s returned=%s uris=%s",
            user_id,
            session_id,
            len(ranked),
            [memory.uri for memory in ranked],
        )
        return ranked

    async def retrieve_recent_session_turns(
        self,
        *,
        user_id: str,
        session_id: str,
        limit: int | None = None,
    ) -> list[SessionTurn]:
        turn_limit = limit or self._settings.memory_recent_session_turn_limit
        if turn_limit <= 0:
            return []

        if self._client is None:
            logger.info(
                "retrieve_recent_session_turns openviking_client_unavailable session_id=%s",
                session_id,
            )
            logger.warning(
                "Recent session history is unavailable because OpenViking is unavailable session_id=%s user_id=%s",
                session_id,
                user_id,
            )
            return []

        session = self._get_or_create_session(session_id, user_id)
        turns = await self._load_openviking_session_turns(session)
        logger.info(
            "retrieve_recent_session_turns session_id=%s source=openviking count=%s",
            session_id,
            min(len(turns), turn_limit),
        )
        return turns[-turn_limit:]

    async def promote_memories(
        self,
        *,
        user_id: str,
        session_id: str,
        request_text: str,
        response_text: str,
        route: AgentRoute,
        used_tools: list[ToolName],
    ) -> list[str]:
        logger.info(
            "promote_memories user_id=%s session_id=%s route=%s request_len=%s response_len=%s used_tools=%s",
            user_id,
            session_id,
            route.value,
            len(request_text),
            len(response_text),
            len(used_tools),
        )
        if self._client is None:
            logger.info(
                "promote_memories openviking_client_unavailable session_id=%s",
                session_id,
            )
            logger.warning(
                "Promoted memories are being dropped because OpenViking is unavailable session_id=%s user_id=%s",
                session_id,
                user_id,
            )
            return []

        # Reuse the same session object that performed the writes this turn.
        # Creating a fresh object and committing it risks committing an empty
        # in-memory session if the write hasn't fully flushed or if the SDK
        # tracks dirty state on the object that performed the writes.
        session = self._get_or_create_session(session_id, user_id)
        before_snapshot = self._memory_snapshot(user_id=user_id)
        commit_result = await self._commit_session(session)
        after_snapshot = self._memory_snapshot(user_id=user_id)
        promoted = _changed_memory_uris(before_snapshot, after_snapshot)
        logger.info(
            "promote_memories completed session_id=%s extracted=%s promoted=%s",
            session_id,
            _commit_memories_extracted(commit_result),
            len(promoted),
        )
        return promoted

    async def finalize_turn(
        self,
        *,
        user_id: str,
        session_id: str,
        request_text: str,
        response_text: str,
        route: AgentRoute,
        used_memory_uris: list[str],
        used_tools: list[ToolName],
    ) -> list[str]:
        logger.info(
            "finalize_turn user_id=%s session_id=%s route=%s used_memory=%s used_tools=%s",
            user_id,
            session_id,
            route.value,
            len(used_memory_uris),
            len(used_tools),
        )
        promoted: list[str] = []
        if self._client is None:
            logger.info(
                "finalize_turn openviking_client_unavailable session_id=%s",
                session_id,
            )
            logger.warning(
                "finalize_turn cannot persist session memory because OpenViking is unavailable session_id=%s user_id=%s",
                session_id,
                user_id,
            )
            return []

        session = self._get_or_create_session(session_id, user_id)
        logger.info(
            "finalize_turn session_backend=openviking session_id=%s used_memory=%s",
            session_id,
            len(used_memory_uris),
        )
        await self._mark_session_usage(session, used_memory_uris)
        if self._settings.memory_commit_enabled and self._settings.memory_use_openviking_commit:
            promoted = await self.promote_memories(
                user_id=user_id,
                session_id=session_id,
                request_text=request_text,
                response_text=response_text,
                route=route,
                used_tools=used_tools,
            )

        # Release the session object after the turn is fully finalized.
        # Next turn will get a fresh object that loads from persisted storage.
        scoped_id = _scoped_session_id(user_id, session_id)
        self._active_sessions.pop(scoped_id, None)
        logger.info(
            "finalize_turn native_openviking_complete session_id=%s route=%s promoted=%s",
            session_id,
            route.value,
            len(promoted),
        )
        return promoted

    def _get_or_create_session(self, session_id: str, user_id: str) -> OpenVikingSession:
        """Return the cached session object for this turn, or create one if needed.

        Reusing the same object across append_user_turn, append_assistant_turn,
        and promote_memories ensures the SDK's in-memory dirty state is consistent
        and commit() sees all messages written during this turn.
        """
        if self._client is None:
            raise RuntimeError("OpenVikingMemoryStore is not initialized")
        scoped_id = _scoped_session_id(user_id, session_id)
        if scoped_id not in self._active_sessions:
            self._active_sessions[scoped_id] = self._client.session(session_id=scoped_id)
            logger.info(
                "session_created scoped_id=%s",
                scoped_id,
            )
        return self._active_sessions[scoped_id]

    async def _append_session_message(
        self, session: OpenVikingSession, role: str, content: str
    ) -> None:
        # add_message requires a List[TextPart | ContextPart | ToolPart].
        # Passing a plain string or dict causes content to be lost.
        parts = [TextPart(text=content)]
        call_result = session.add_message(role, parts)
        await _await_if_needed(call_result)

    async def _load_openviking_session_turns(self, session: OpenVikingSession) -> list[SessionTurn]:
        messages = getattr(session, "messages", None)
        turns = _coerce_session_turns(messages)
        if turns:
            return turns

        try:
            loaded = session.load()
            loaded_payload = await _await_value_if_needed(loaded)
        except Exception as exc:
            logger.warning(
                "openviking session load failed session_id=%s error=%s",
                getattr(session, "session_id", "unknown"),
                exc,
            )
            return []

        if isinstance(loaded_payload, dict):
            return _coerce_session_turns(loaded_payload.get("messages"))
        return []

    async def _coerce_openviking_search_results(self, raw_result: object) -> list[RetrievedMemory]:
        results = _extract_search_items(raw_result)
        memories: list[RetrievedMemory] = []
        for item in results:
            uri = _search_item_uri(item)
            if not uri:
                continue

            score = _search_item_score(item)
            abstract = ""
            overview = ""
            detail = ""
            with contextlib.suppress(Exception):
                if self._client is not None:
                    abstract = (await self._client.abstract(uri)).strip()
            with contextlib.suppress(Exception):
                if self._client is not None:
                    overview = (await self._client.overview(uri)).strip()
            if not overview:
                with contextlib.suppress(Exception):
                    if self._client is not None:
                        detail = (await self._client.read(uri)).strip()

            content = overview or detail or abstract
            if not content:
                continue

            memories.append(
                RetrievedMemory(
                    uri=uri,
                    bucket=_uri_bucket(uri),
                    kind=_uri_memory_kind(uri),
                    abstract=abstract or content,
                    content=content,
                    score=score,
                    importance=0.5,
                    confidence=0.5,
                    age_days=0.0,
                    is_expired=False,
                )
            )
        return memories[: self._settings.memory_max_retrieval_count]

    async def _mark_session_usage(
        self, session: OpenVikingSession, used_memory_uris: list[str]
    ) -> None:
        if not used_memory_uris:
            return
        used_method = getattr(session, "used", None)
        if used_method is None:
            logger.info(
                "finalize_turn session_used_unsupported session_id=%s used_memory=%s",
                getattr(session, "session_id", "unknown"),
                len(used_memory_uris),
            )
            return
        try:
            call_result = used_method(contexts=used_memory_uris)
        except TypeError:
            try:
                call_result = used_method(used_memory_uris)
            except TypeError:
                for uri in used_memory_uris:
                    try:
                        call_result = used_method(uri)
                    except TypeError:
                        call_result = used_method(context_uri=uri)
                    await _await_if_needed(call_result)
            else:
                await _await_if_needed(call_result)
        else:
            await _await_if_needed(call_result)
        logger.info(
            "finalize_turn session_used_recorded session_id=%s used_memory=%s",
            getattr(session, "session_id", "unknown"),
            len(used_memory_uris),
        )

    async def _commit_session(self, session: OpenVikingSession) -> object:
        return await _await_value_if_needed(session.commit())

    def _memory_snapshot(self, *, user_id: str) -> dict[str, int]:
        # OpenViking writes memories under viking/user/default/... and
        # viking/agent/{agent_id}/... not viking/default/user/{user_id}/...
        roots = [
            self._base / "viking" / "user" / "default" / "memories",
            self._base / "viking" / "agent",
        ]
        snapshot: dict[str, int] = {}
        for root in roots:
            if not root.exists():
                continue
            for path in root.rglob("*.md"):
                if path.name.startswith("."):
                    continue
                uri = self._path_to_viking_uri(path)
                if uri is None:
                    continue
                snapshot[uri] = path.stat().st_mtime_ns
        return snapshot

    def _path_to_viking_uri(self, path: Path) -> str | None:
        try:
            relative = path.relative_to(self._base / "viking" / "default")
        except ValueError:
            return None
        return f"viking://{relative.as_posix()}"

    def _ensure_openviking_config(self) -> None:
        config_path = self._base / "ov.conf"
        embedding_config = self._openviking_embedding_config()
        vlm_config = self._openviking_vlm_config()

        config: dict[str, object] = {
            "storage": {"workspace": str(self._base)},
            "default_search_mode": "fast",
            "default_search_limit": 4,
        }
        if embedding_config is not None:
            config["embedding"] = {"dense": embedding_config}
        if vlm_config is not None:
            config["vlm"] = vlm_config

        config_path.write_text(json.dumps(config, indent=2), encoding="utf-8")
        os.environ["OPENVIKING_CONFIG_FILE"] = str(config_path)
        logger.info(
            "openviking config written path=%s provider=%s model=%s dimension=%s vlm_provider=%s vlm_model=%s",
            config_path,
            embedding_config.get("provider") if embedding_config else "none",
            embedding_config.get("model") if embedding_config else "none",
            embedding_config.get("dimension") if embedding_config else "none",
            vlm_config.get("provider") if vlm_config else "none",
            vlm_config.get("model") if vlm_config else "none",
        )

    def _has_openviking_embedding_config(self) -> bool:
        return self._openviking_embedding_config() is not None

    def _openviking_embedding_config(self) -> dict[str, object] | None:
        if self._settings.jina_api_key:
            logger.info(
                "openviking embedding config selected provider=jina model=%s dimension=%s",
                self._settings.jina_embedding_model,
                self._settings.jina_embedding_dimension,
            )
            return {
                "provider": "jina",
                "api_key": self._settings.jina_api_key,
                "model": self._settings.jina_embedding_model,
                "dimension": self._settings.jina_embedding_dimension,
            }
        if self._settings.openrouter_api_key:
            logger.info(
                "openviking embedding config selected provider=openai-compatible model=%s dimension=%s",
                self._settings.openrouter_embedding_model,
                1536,
            )
            return {
                "provider": "openai",
                "model": self._settings.openrouter_embedding_model,
                "api_key": self._settings.openrouter_api_key,
                "api_base": _openrouter_api_base(self._settings.openrouter_base_url),
                "dimension": 1536,
            }
        return None

    def _openviking_vlm_config(self) -> dict[str, object] | None:
        if self._settings.groq_api_key:
            # LiteLLM (used internally by OpenViking) requires a provider-prefixed
            # model name for Groq: "groq/<model>" not just "<model>".
            model = self._settings.groq_model
            prefixed_model = model if model.startswith("groq/") else f"groq/{model}"
            logger.info(
                "openviking vlm config selected provider=groq model=%s",
                prefixed_model,
            )
            return {
                "provider": "groq",
                "api_key": self._settings.groq_api_key,
                "model": prefixed_model,
            }
        if self._settings.openrouter_api_key:
            logger.info(
                "openviking vlm config selected provider=openai-compatible model=%s",
                self._settings.openrouter_chat_model,
            )
            return {
                "provider": "openai",
                "api_key": self._settings.openrouter_api_key,
                "api_base": self._settings.openrouter_base_url,
                "model": self._settings.openrouter_chat_model,
            }
        logger.warning(
            "openviking vlm config: no LLM provider found, memory extraction will be skipped"
        )
        return None


def _scoped_session_id(user_id: str, session_id: str) -> str:
    return f"{user_id}:{session_id}" if user_id else session_id


async def _await_if_needed(result: object) -> None:
    if inspect.isawaitable(result):
        await result


async def _await_value_if_needed(result: object) -> object:
    if inspect.isawaitable(result):
        return await result
    return result


def _coerce_session_turns(raw_messages: object) -> list[SessionTurn]:
    if not isinstance(raw_messages, list):
        return []

    turns: list[SessionTurn] = []
    for message in raw_messages:
        role = _message_attr(message, "role")
        content = _message_content(message)
        created_at = _message_attr(message, "created_at")
        if not isinstance(role, str) or not isinstance(content, str) or not content.strip():
            continue
        turns.append(
            SessionTurn(
                role=role,
                content=content,
                created_at=created_at if isinstance(created_at, str) else None,
            )
        )
    return turns


def _message_attr(message: object, name: str) -> object:
    if isinstance(message, dict):
        return message.get(name)
    return getattr(message, name, None)


def _message_content(message: object) -> object:
    content = _message_attr(message, "content")
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        content_parts: list[str] = []
        for item in content:
            if isinstance(item, str):
                content_parts.append(item)
            elif isinstance(item, dict):
                text = item.get("text")
                if isinstance(text, str):
                    content_parts.append(text)
        return "\n".join(part for part in content_parts if part.strip())
    message_parts = _message_attr(message, "parts")
    if isinstance(message_parts, list):
        text_parts: list[str] = []
        for item in message_parts:
            if isinstance(item, dict):
                text = item.get("text")
                if isinstance(text, str):
                    text_parts.append(text)
        if text_parts:
            return "\n".join(part for part in text_parts if part.strip())
    return content


def _extract_search_items(raw_result: object) -> list[object]:
    if isinstance(raw_result, list):
        return raw_result
    if isinstance(raw_result, dict):
        for key in ("items", "results", "hits", "contexts", "data", "memories"):
            value = raw_result.get(key)
            if isinstance(value, list):
                return value
    # FindResult from openviking_cli.retrieve.types has a .memories attribute
    # containing MatchedContext objects — this is the primary result container.
    value = getattr(raw_result, "memories", None)
    if isinstance(value, list):
        return value
    value = getattr(raw_result, "items", None)
    if isinstance(value, list):
        return value
    value = getattr(raw_result, "results", None)
    if isinstance(value, list):
        return value
    return []


def _search_item_uri(item: object) -> str | None:
    if isinstance(item, dict):
        for key in ("uri", "context_uri", "resource_uri", "path"):
            value = item.get(key)
            if isinstance(value, str) and value:
                return value
    for key in ("uri", "context_uri", "resource_uri", "path"):
        value = getattr(item, key, None)
        if isinstance(value, str) and value:
            return value
    return None


def _search_item_score(item: object) -> float:
    if isinstance(item, dict):
        for key in ("score", "similarity", "final_score"):
            value = item.get(key)
            if isinstance(value, int | float):
                return float(value)
    for key in ("score", "similarity", "final_score"):
        value = getattr(item, key, None)
        if isinstance(value, int | float):
            return float(value)
    return 0.0


def _uri_bucket(uri: str) -> str:
    parts = uri.split("/")
    if len(parts) >= 5:
        return parts[4]
    return "contexts"


def _uri_memory_kind(uri: str) -> MemoryKind:
    bucket = _uri_bucket(uri)
    return {
        "preferences": MemoryKind.PREFERENCE,
        "entities": MemoryKind.PATTERN,
        "goals": MemoryKind.GOAL,
        "patterns": MemoryKind.PATTERN,
        "events": MemoryKind.EVENT,
    }.get(bucket, MemoryKind.PATTERN)


def _openrouter_api_base(url: str) -> str:
    if url.endswith("/embeddings"):
        return url[: -len("/embeddings")]
    return url


def _changed_memory_uris(before: dict[str, int], after: dict[str, int]) -> list[str]:
    return sorted(uri for uri, mtime in after.items() if before.get(uri) != mtime)


def _commit_memories_extracted(result: object) -> int:
    if isinstance(result, dict):
        value = result.get("memories_extracted")
        if isinstance(value, int):
            return value
    return 0