from __future__ import annotations

import hashlib
import logging

from langchain_openai import OpenAIEmbeddings

from tradstry_agents.config import Settings

logger = logging.getLogger(__name__)


class OpenRouterEmbeddingProvider:
    def __init__(self, settings: Settings):
        self._settings = settings
        self._embeddings = (
            OpenAIEmbeddings(
                model=self._settings.openrouter_embedding_model,
                api_key=self._settings.openrouter_api_key,
                base_url=_openrouter_base_url(self._settings.openrouter_base_url),
                max_retries=1,
                request_timeout=self._settings.response_timeout_seconds,
            )
            if self._settings.openrouter_api_key
            else None
        )

    async def embed_text(self, text: str) -> list[float]:
        if self._embeddings is None:
            return self._fallback_embedding(text)

        try:
            return await self._embeddings.aembed_query(text)
        except Exception as exc:
            logger.warning("OpenRouter embedding request failed via LangChain: %s", exc)
            return self._fallback_embedding(text)

    def _fallback_embedding(self, text: str) -> list[float]:
        digest = hashlib.sha256(text.encode("utf-8")).digest()
        # Small deterministic vector for local ranking when OpenRouter is absent.
        return [byte / 255.0 for byte in digest[:32]]


def _openrouter_base_url(url: str) -> str:
    normalized = url.rstrip("/")
    if normalized.endswith("/embeddings"):
        return normalized[: -len("/embeddings")]
    return normalized
