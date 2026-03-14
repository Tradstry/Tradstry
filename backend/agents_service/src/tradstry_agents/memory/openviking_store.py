from __future__ import annotations

import contextlib
import inspect
import json
import math
import os
import re
from dataclasses import dataclass
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Protocol, TypedDict, cast
from uuid import uuid4

from openviking import AsyncOpenViking  # type: ignore[import-untyped]

from tradstry_agents.config import Settings
from tradstry_agents.providers import EmbeddingProvider
from tradstry_agents.schemas import AgentRoute, MemoryKind, ToolName


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


class OpenVikingSession(Protocol):
    def add_message(self, role: str, /, *args: object, **kwargs: object) -> object:
        ...


class OpenVikingClient(Protocol):
    def session(self, *, session_id: str) -> OpenVikingSession:
        ...

    async def initialize(self) -> None:
        ...

    async def close(self) -> None:
        ...


class _StoredMetadata(TypedDict, total=False):
    uri: str
    bucket: str
    kind: str
    title: str
    created_at: str
    updated_at: str
    expires_at: str
    superseded_by: str
    source_session_id: str
    route: str
    used_tools: list[str]
    source_text: str
    importance: float
    confidence: float
    vector: list[float]


class _MemoryCandidate(TypedDict):
    kind: MemoryKind
    bucket: str
    title: str
    abstract: str
    content: str
    importance: float
    confidence: float
    expires_at: str | None


class _DocumentRecord(TypedDict):
    root_path: str
    uri: str
    bucket: str
    kind: MemoryKind
    abstract: str
    content: str
    vector: list[float]
    importance: float
    confidence: float
    created_at: str
    updated_at: str
    expires_at: str | None
    superseded_by: str | None


class _FallbackSessionMessage(TypedDict):
    id: str
    role: str
    content: str
    created_at: str


class OpenVikingMemoryStore:
    def __init__(self, settings: Settings, embedding_provider: EmbeddingProvider):
        self._settings = settings
        self._embedding_provider = embedding_provider
        self._client: OpenVikingClient | None = None
        self._base = settings.openviking_data_root

    async def initialize(self) -> None:
        self._base.mkdir(parents=True, exist_ok=True)
        for bucket in ("session", "user", "agent", "queue", "index"):
            (self._base / bucket).mkdir(parents=True, exist_ok=True)
        if self._settings.openrouter_api_key:
            self._ensure_openviking_config()
            self._client = AsyncOpenViking(path=str(self._base))
            await self._client.initialize()

    async def close(self) -> None:
        if self._client is not None:
            await self._client.close()
            self._client = None

    async def append_user_turn(self, *, user_id: str, session_id: str, content: str) -> None:
        if self._client is not None:
            session = self._session(session_id)
            await self._append_session_message(session, "user", content)
            return
        self._append_fallback_session_line(
            user_id=user_id,
            session_id=session_id,
            role="user",
            content=content,
        )

    async def append_assistant_turn(self, *, user_id: str, session_id: str, content: str) -> None:
        if self._client is not None:
            session = self._session(session_id)
            await self._append_session_message(session, "assistant", content)
            return
        self._append_fallback_session_line(
            user_id=user_id,
            session_id=session_id,
            role="assistant",
            content=content,
        )

    async def _append_session_message(self, session: OpenVikingSession, role: str, content: str) -> None:
        call_result: object
        try:
            call_result = session.add_message(role, content=content)
        except TypeError:
            call_result = session.add_message(role, content)
        await _await_if_needed(call_result)

    async def retrieve_context(
        self, *, user_id: str, session_id: str, query: str, limit: int | None = None
    ) -> list[RetrievedMemory]:
        del session_id
        documents = await self._load_memory_documents(user_id=user_id)
        if not documents:
            return []

        query_vector = await self._embedding_provider.embed_text(query)
        ranked: list[RetrievedMemory] = []
        now = datetime.now(UTC)
        for document in documents:
            if document["superseded_by"]:
                continue
            age_days = _memory_age_days(document, now)
            is_expired = _is_expired(document, now)
            if is_expired:
                continue
            score = _cosine_similarity(query_vector, document["vector"])
            freshness_score = _freshness_score(document, now, self._settings)
            blended_score = (
                (score * 0.75)
                + (freshness_score * 0.15)
                + (document["importance"] * 0.10)
            )
            ranked.append(
                RetrievedMemory(
                    uri=document["uri"],
                    bucket=document["bucket"],
                    kind=document["kind"],
                    abstract=document["abstract"],
                    content=document["content"],
                    score=blended_score,
                    importance=document["importance"],
                    confidence=document["confidence"],
                    age_days=age_days,
                    is_expired=is_expired,
                )
            )

        ranked.sort(key=lambda item: item.score, reverse=True)
        retrieval_limit = limit or self._settings.memory_max_retrieval_count
        return ranked[:retrieval_limit]

    async def promote_memories(
        self, *, user_id: str, request_text: str, response_text: str
    ) -> list[str]:
        return await self.finalize_turn(
            user_id=user_id,
            session_id="promote_memories",
            request_text=request_text,
            response_text=response_text,
            route=AgentRoute.TRADING_EDUCATOR,
            used_memory_uris=[],
            used_tools=[],
        )

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
        if self._client is not None:
            session = self._session(session_id)
            await self._mark_session_usage(session, used_memory_uris)
            if self._settings.memory_commit_enabled and self._settings.memory_use_openviking_commit:
                await self._commit_session(session)

        if not response_text.strip():
            return []

        candidates = self._build_memory_candidates(
            request_text=request_text,
            response_text=response_text,
            route=route,
        )
        stored: list[str] = []
        for candidate in candidates:
            uri = await self._upsert_memory_doc(
                user_id=user_id,
                session_id=session_id,
                route=route,
                used_tools=used_tools,
                candidate=candidate,
            )
            stored.append(uri)
        return stored

    def _session(self, session_id: str) -> OpenVikingSession:
        if self._client is None:
            raise RuntimeError("OpenVikingMemoryStore is not initialized")
        return self._client.session(session_id=session_id)

    def _append_fallback_session_line(
        self, *, user_id: str, session_id: str, role: str, content: str
    ) -> None:
        session_root = self._base / "session" / user_id / session_id
        session_root.mkdir(parents=True, exist_ok=True)
        messages_path = session_root / "messages.jsonl"
        payload: _FallbackSessionMessage = {
            "id": f"msg_{uuid4().hex}",
            "role": role,
            "content": content,
            "created_at": datetime.now(UTC).isoformat(),
        }
        with messages_path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(payload) + "\n")

    async def _load_memory_documents(self, *, user_id: str) -> list[_DocumentRecord]:
        results: list[_DocumentRecord] = []
        user_root = self._base / "user" / user_id
        if not user_root.exists():
            return results

        for detail_path in user_root.glob("*/*/detail.md"):
            bucket = detail_path.parents[1].name
            abstract_path = detail_path.with_name(".abstract.md")
            meta_path = detail_path.with_name(".meta.json")
            if not abstract_path.exists() or not meta_path.exists():
                continue

            metadata = _coerce_dict(meta_path.read_text(encoding="utf-8"))
            if not metadata:
                continue

            uri = metadata.get("uri")
            if not isinstance(uri, str):
                continue
            kind = _coerce_memory_kind(metadata.get("kind"), bucket)

            abstract = abstract_path.read_text(encoding="utf-8").strip()
            if not abstract:
                continue

            vector = _coerce_vector(metadata.get("vector"))
            if not vector:
                continue

            content = detail_path.read_text(encoding="utf-8").strip()
            results.append(
                _DocumentRecord(
                    root_path=str(detail_path.parent),
                    uri=uri,
                    bucket=bucket,
                    kind=kind,
                    abstract=abstract,
                    content=content,
                    vector=vector,
                    importance=_coerce_float(metadata.get("importance"), default=0.5),
                    confidence=_coerce_float(metadata.get("confidence"), default=0.5),
                    created_at=_coerce_timestamp(
                        metadata.get("created_at"), default=datetime.now(UTC).isoformat()
                    ),
                    updated_at=_coerce_timestamp(
                        metadata.get("updated_at"), default=datetime.now(UTC).isoformat()
                    ),
                    expires_at=_coerce_optional_timestamp(metadata.get("expires_at")),
                    superseded_by=_coerce_optional_str(metadata.get("superseded_by")),
                )
            )
        return results

    async def _upsert_memory_doc(
        self,
        *,
        user_id: str,
        session_id: str,
        route: AgentRoute,
        used_tools: list[ToolName],
        candidate: _MemoryCandidate,
    ) -> str:
        vector = await self._embedding_provider.embed_text(
            f"{candidate['abstract']}\n\n{candidate['content']}"
        )
        existing = await self._find_existing_memory(
            user_id=user_id,
            candidate=candidate,
            candidate_vector=vector,
        )
        if existing is not None and candidate["kind"] is not MemoryKind.EVENT:
            doc_root = existing["root_path"]
            uri = existing["uri"]
            created_at = existing["created_at"]
            content = _merge_memory_content(existing["content"], candidate["content"])
        else:
            slug = _slugify(candidate["title"]) or uuid4().hex
            doc_root = str(self._base / "user" / user_id / candidate["bucket"] / slug)
            uri = f"viking://user/{user_id}/{candidate['bucket']}/{slug}"
            created_at = datetime.now(UTC).isoformat()
            content = candidate["content"]

        doc_root_path = self._base / "user" / user_id / candidate["bucket"] / Path(doc_root).name
        doc_root_path.mkdir(parents=True, exist_ok=True)
        metadata: _StoredMetadata = {
            "uri": uri,
            "bucket": candidate["bucket"],
            "kind": candidate["kind"].value,
            "title": candidate["title"],
            "created_at": created_at,
            "updated_at": datetime.now(UTC).isoformat(),
            "expires_at": candidate["expires_at"] or "",
            "source_session_id": session_id,
            "route": route.value,
            "used_tools": [tool for tool in used_tools],
            "source_text": candidate["content"],
            "importance": candidate["importance"],
            "confidence": candidate["confidence"],
            "vector": vector,
        }
        if existing is not None and candidate["kind"] is not MemoryKind.EVENT:
            metadata["superseded_by"] = ""
        (doc_root_path / ".meta.json").write_text(json.dumps(metadata, indent=2), encoding="utf-8")
        (doc_root_path / ".abstract.md").write_text(candidate["abstract"].strip(), encoding="utf-8")
        (doc_root_path / ".overview.md").write_text(content.strip(), encoding="utf-8")
        (doc_root_path / "detail.md").write_text(content.strip(), encoding="utf-8")
        return uri

    async def _find_existing_memory(
        self,
        *,
        user_id: str,
        candidate: _MemoryCandidate,
        candidate_vector: list[float],
    ) -> _DocumentRecord | None:
        if candidate["kind"] is MemoryKind.EVENT:
            return None
        documents = await self._load_memory_documents(user_id=user_id)
        matches = [
            document
            for document in documents
            if document["kind"] is candidate["kind"] and not document["superseded_by"]
        ]
        best_match: _DocumentRecord | None = None
        best_score = 0.0
        for document in matches:
            score = _cosine_similarity(candidate_vector, document["vector"])
            if score >= self._settings.memory_similarity_merge_threshold and score > best_score:
                best_match = document
                best_score = score
        return best_match

    def _build_memory_candidates(
        self,
        *,
        request_text: str,
        response_text: str,
        route: AgentRoute,
    ) -> list[_MemoryCandidate]:
        text = request_text.strip()
        lowered = text.lower()
        candidates: list[_MemoryCandidate] = []
        response_excerpt = response_text.strip()[:400]

        if any(token in lowered for token in ("prefer", "i like", "keep it concise", "be brief")):
            candidates.append(
                {
                    "kind": MemoryKind.PREFERENCE,
                    "bucket": "preferences",
                    "title": "answer-style",
                    "abstract": "The user has expressed a response style preference.",
                    "content": text,
                    "importance": 0.85,
                    "confidence": 0.80,
                    "expires_at": None,
                }
            )

        if any(token in lowered for token in ("goal", "focus on", "working on", "i want to improve")):
            candidates.append(
                {
                    "kind": MemoryKind.GOAL,
                    "bucket": "goals",
                    "title": "active-trading-goal",
                    "abstract": "The user described an active trading improvement goal.",
                    "content": text,
                    "importance": 0.90,
                    "confidence": 0.75,
                    "expires_at": _future_iso(days=self._settings.memory_goal_ttl_days),
                }
            )

        if any(token in lowered for token in ("i usually", "i always", "i keep", "my pattern")):
            candidates.append(
                {
                    "kind": MemoryKind.PATTERN,
                    "bucket": "patterns",
                    "title": "trading-pattern",
                    "abstract": "The user described a recurring trading pattern.",
                    "content": text,
                    "importance": 0.80,
                    "confidence": 0.70,
                    "expires_at": None,
                }
            )

        if any(token in lowered for token in ("today", "this week", "i broke", "i missed")):
            candidates.append(
                {
                    "kind": MemoryKind.EVENT,
                    "bucket": "events",
                    "title": "trading-event",
                    "abstract": "The user described a significant recent trading event.",
                    "content": (
                        f"User message:\n{text}\n\nAssistant response excerpt:\n{response_excerpt}\n\n"
                        f"Route: {route.value}"
                    ).strip(),
                    "importance": 0.70,
                    "confidence": 0.65,
                    "expires_at": _future_iso(days=self._settings.memory_event_ttl_days),
                }
            )

        return candidates

    async def _mark_session_usage(
        self, session: OpenVikingSession, used_memory_uris: list[str]
    ) -> None:
        used_method = getattr(session, "used", None)
        if used_method is None:
            return
        for uri in used_memory_uris:
            try:
                call_result = used_method(uri)
            except TypeError:
                call_result = used_method(context_uri=uri)
            await _await_if_needed(call_result)

    async def _commit_session(self, session: OpenVikingSession) -> None:
        commit_method = getattr(session, "commit", None)
        if commit_method is None:
            return
        await _await_if_needed(commit_method())

    def _ensure_openviking_config(self) -> None:
        config_path = self._base / "ov.conf"
        if not config_path.exists():
            config = {
                "storage": {"workspace": str(self._base)},
                "embedding": {
                    "dense": {
                        "provider": "openai",
                        "model": self._settings.openrouter_embedding_model,
                        "api_key": self._settings.openrouter_api_key or "local-dev-placeholder",
                        "api_base": _openrouter_api_base(self._settings.openrouter_base_url),
                        "dimension": 1536,
                    }
                },
                "default_search_mode": "fast",
                "default_search_limit": 4,
            }
            config_path.write_text(json.dumps(config, indent=2), encoding="utf-8")
        os.environ.setdefault("OPENVIKING_CONFIG_FILE", str(config_path))


async def _await_if_needed(result: object) -> None:
    if inspect.isawaitable(result):
        await result


def _coerce_dict(raw_payload: str) -> _StoredMetadata:
    try:
        parsed = json.loads(raw_payload)
    except json.JSONDecodeError:
        return {}
    if not isinstance(parsed, dict):
        return {}
    return cast(_StoredMetadata, parsed)


def _coerce_vector(value: object) -> list[float]:
    if not isinstance(value, list):
        return []

    vectors: list[float] = []
    for item in value:
        if isinstance(item, int | float):
            vectors.append(float(item))
    return vectors


def _slugify(value: str) -> str:
    lowered = value.lower().strip()
    return re.sub(r"[^a-z0-9]+", "-", lowered).strip("-")


def _openrouter_api_base(url: str) -> str:
    if url.endswith("/embeddings"):
        return url[: -len("/embeddings")]
    return url


def _cosine_similarity(left: list[float], right: list[float]) -> float:
    if not left or not right:
        return 0.0
    size = min(len(left), len(right))
    left = left[:size]
    right = right[:size]
    numerator = sum(a * b for a, b in zip(left, right))
    left_norm = math.sqrt(sum(a * a for a in left))
    right_norm = math.sqrt(sum(b * b for b in right))
    if left_norm == 0 or right_norm == 0:
        return 0.0
    return numerator / (left_norm * right_norm)


def _coerce_memory_kind(value: object, bucket: str) -> MemoryKind:
    if isinstance(value, str):
        with contextlib.suppress(ValueError):
            return MemoryKind(value)
    return {
        "preferences": MemoryKind.PREFERENCE,
        "goals": MemoryKind.GOAL,
        "patterns": MemoryKind.PATTERN,
        "events": MemoryKind.EVENT,
    }.get(bucket, MemoryKind.PATTERN)


def _coerce_float(value: object, *, default: float) -> float:
    if isinstance(value, int | float):
        return float(value)
    return default


def _coerce_optional_str(value: object) -> str | None:
    return value if isinstance(value, str) and value.strip() else None


def _coerce_timestamp(value: object, *, default: str) -> str:
    return value if isinstance(value, str) and value.strip() else default


def _coerce_optional_timestamp(value: object) -> str | None:
    return value if isinstance(value, str) and value.strip() else None


def _parse_datetime(value: str | None) -> datetime | None:
    if value is None:
        return None
    with contextlib.suppress(ValueError):
        return datetime.fromisoformat(value)
    return None


def _future_iso(*, days: int) -> str:
    return (datetime.now(UTC) + timedelta(days=days)).replace(microsecond=0).isoformat()


def _memory_age_days(document: _DocumentRecord, now: datetime) -> float:
    updated_at = _parse_datetime(document["updated_at"]) or _parse_datetime(document["created_at"])
    if updated_at is None:
        return 0.0
    return max(0.0, (now - updated_at).total_seconds() / 86400.0)


def _is_expired(document: _DocumentRecord, now: datetime) -> bool:
    expires_at = _parse_datetime(document["expires_at"])
    return expires_at is not None and expires_at <= now


def _freshness_score(document: _DocumentRecord, now: datetime, settings: Settings) -> float:
    age_days = _memory_age_days(document, now)
    if document["kind"] is MemoryKind.PREFERENCE:
        horizon_days = 3650.0
    elif document["kind"] is MemoryKind.GOAL:
        horizon_days = float(settings.memory_goal_ttl_days)
    elif document["kind"] is MemoryKind.EVENT:
        horizon_days = float(settings.memory_event_ttl_days)
    else:
        horizon_days = 365.0
    if horizon_days <= 0:
        return 0.0
    return math.exp(-settings.memory_recency_decay_factor * (age_days / horizon_days))


def _merge_memory_content(existing_content: str, new_content: str) -> str:
    normalized_existing = existing_content.strip()
    normalized_new = new_content.strip()
    if not normalized_existing:
        return normalized_new
    if normalized_new in normalized_existing:
        return normalized_existing
    return f"{normalized_existing}\n\nUpdate:\n{normalized_new}"
