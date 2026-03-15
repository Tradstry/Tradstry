from __future__ import annotations

from collections.abc import AsyncIterator
import logging
from textwrap import shorten

from langchain_groq import ChatGroq
from langchain_core.messages import AIMessage, AIMessageChunk

from tradstry_agents.config import Settings

logger = logging.getLogger(__name__)


class GroqChatProvider:
    def __init__(self, settings: Settings):
        self._settings = settings
        self._model = (
            ChatGroq(
                model_name=self._settings.groq_model,
                temperature=0.2,
                request_timeout=self._settings.response_timeout_seconds,
                max_retries=1,
                groq_api_key=self._settings.groq_api_key,
                groq_api_base=_groq_api_base(self._settings.groq_base_url),
            )
            if self._settings.groq_api_key
            else None
        )

    async def complete(self, *, system_prompt: str, user_prompt: str) -> str:
        parts: list[str] = []
        async for chunk in self.stream_complete(
            system_prompt=system_prompt,
            user_prompt=user_prompt,
        ):
            parts.append(chunk)
        return "".join(parts).strip()

    async def stream_complete(
        self, *, system_prompt: str, user_prompt: str
    ) -> AsyncIterator[str]:
        if self._model is None:
            yield self._fallback_response(user_prompt, reason="not_configured")
            return

        try:
            async for chunk in self._model.astream(
                [
                    ("system", system_prompt),
                    ("user", user_prompt),
                ]
            ):
                text = _chunk_text(chunk)
                if text:
                    yield text
            return
        except Exception as exc:
            logger.warning("Groq chat streaming request failed via LangChain: %s", exc)
            yield self._fallback_response(user_prompt, reason="request_failed")
            return

    def _fallback_response(
        self,
        user_prompt: str,
        *,
        reason: str,
    ) -> str:
        preview = shorten(user_prompt.replace("\n", " "), width=320, placeholder="...")
        if reason == "not_configured":
            intro = "Groq is not configured, so this is a deterministic fallback answer."
        elif reason == "request_failed":
            intro = "Groq is temporarily unavailable, so this is a deterministic fallback answer."
        else:
            intro = "Groq returned an invalid response, so this is a deterministic fallback answer."
        return (
            f"{intro}\n\n"
            f"Request summary: {preview}"
        )


def _groq_api_base(url: str) -> str:
    normalized = url.rstrip("/")
    if normalized.endswith("/openai/v1/chat/completions"):
        return normalized[: -len("/openai/v1/chat/completions")]
    if normalized.endswith("/openai/v1"):
        return normalized[: -len("/openai/v1")]
    if normalized.endswith("/chat/completions"):
        return normalized[: -len("/chat/completions")]
    return normalized


def _message_text(message: AIMessage) -> str:
    content = message.content
    if isinstance(content, str):
        return content
    text_parts: list[str] = []
    for item in content:
        if isinstance(item, str):
            text_parts.append(item)
        elif isinstance(item, dict):
            text_value = item.get("text")
            if isinstance(text_value, str):
                text_parts.append(text_value)
    text = "\n".join(part for part in text_parts if part.strip()).strip()
    if not text:
        raise ValueError("empty AI message content")
    return text


def _chunk_text(message: AIMessageChunk) -> str:
    content = message.content
    if isinstance(content, str):
        return content
    text_parts: list[str] = []
    for item in content:
        if isinstance(item, str):
            text_parts.append(item)
        elif isinstance(item, dict):
            text_value = item.get("text")
            if isinstance(text_value, str):
                text_parts.append(text_value)
    return "".join(text_parts)
