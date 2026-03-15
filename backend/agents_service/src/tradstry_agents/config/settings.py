from __future__ import annotations

from functools import lru_cache
from pathlib import Path

from pydantic import Field, field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


def _service_root() -> Path:
    return Path(__file__).resolve().parents[3]


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file_encoding="utf-8",
        extra="ignore",
    )

    host: str = Field(default="0.0.0.0", validation_alias="AGENTS_SERVICE_HOST")
    port: int = Field(default=8091, validation_alias="AGENTS_SERVICE_PORT")
    websocket_path: str = Field(
        default="/ws/v1/agents",
        validation_alias="AGENTS_SERVICE_WEBSOCKET_PATH",
    )
    prompt_root: Path = Field(
        default_factory=lambda: _service_root() / "src" / "tradstry_agents" / "prompts",
        validation_alias="AGENTS_PROMPT_ROOT",
    )
    groq_api_key: str | None = Field(default=None, validation_alias="GROQ_API_KEY")
    groq_model: str = Field(default="", validation_alias="GROQ_MODEL")
    groq_base_url: str = Field(
        default="https://api.groq.com/openai/v1/chat/completions",
        validation_alias="GROQ_BASE_URL",
    )
    openrouter_api_key: str | None = Field(default=None, validation_alias="OPENROUTER_API_KEY")
    openrouter_embedding_model: str = Field(
        default="text-embedding-3-small",
        validation_alias="OPENROUTER_EMBEDDING_MODEL",
    )
    openrouter_chat_model: str = Field(
        default="openai/gpt-4o-mini",
        validation_alias="OPENROUTER_CHAT_MODEL",
    )
    openrouter_base_url: str = Field(
        default="https://openrouter.ai/api/v1/embeddings",
        validation_alias="OPENROUTER_BASE_URL",
    )
    openviking_data_root: Path = Field(
        default_factory=lambda: _service_root() / "data" / "openviking",
        validation_alias="OPENVIKING_DATA_ROOT",
    )
    openviking_client_enabled: bool = Field(
        default=False,
        validation_alias="AGENTS_OPENVIKING_CLIENT_ENABLED",
    )
    jina_api_key: str | None = Field(default=None, validation_alias="JINA_API_KEY")
    jina_embedding_model: str = Field(
        default="jina-embeddings-v5-text-small",
        validation_alias="JINA_EMBEDDING_MODEL",
    )
    jina_embedding_dimension: int = Field(
        default=1024,
        validation_alias="JINA_EMBEDDING_DIMENSION",
    )
    response_timeout_seconds: float = Field(
        default=60.0,
        validation_alias="AGENTS_RESPONSE_TIMEOUT_SECONDS",
    )
    tool_timeout_seconds: float = Field(
        default=20.0,
        validation_alias="AGENTS_TOOL_TIMEOUT_SECONDS",
    )
    memory_event_ttl_days: int = Field(
        default=14,
        validation_alias="AGENTS_MEMORY_EVENT_TTL_DAYS",
    )
    memory_goal_ttl_days: int = Field(
        default=90,
        validation_alias="AGENTS_MEMORY_GOAL_TTL_DAYS",
    )
    memory_similarity_merge_threshold: float = Field(
        default=0.88,
        validation_alias="AGENTS_MEMORY_SIMILARITY_MERGE_THRESHOLD",
    )
    memory_max_retrieval_count: int = Field(
        default=4,
        validation_alias="AGENTS_MEMORY_MAX_RETRIEVAL_COUNT",
    )
    memory_recent_session_turn_limit: int = Field(
        default=8,
        validation_alias="AGENTS_MEMORY_RECENT_SESSION_TURN_LIMIT",
    )
    memory_recency_decay_factor: float = Field(
        default=1.0,
        validation_alias="AGENTS_MEMORY_RECENCY_DECAY_FACTOR",
    )
    memory_commit_enabled: bool = Field(
        default=True,
        validation_alias="AGENTS_MEMORY_COMMIT_ENABLED",
    )
    memory_use_openviking_commit: bool = Field(
        default=True,
        validation_alias="AGENTS_MEMORY_USE_OPENVIKING_COMMIT",
    )
    heartbeat_interval_seconds: float = Field(
        default=15.0,
        validation_alias="AGENTS_HEARTBEAT_INTERVAL_SECONDS",
    )

    @field_validator("prompt_root", "openviking_data_root", mode="before")
    @classmethod
    def _coerce_path(cls, value: str | Path) -> Path:
        return Path(value).expanduser()


@lru_cache(maxsize=1)
def load_settings() -> Settings:
    return Settings(_env_file=_service_root() / ".env")
