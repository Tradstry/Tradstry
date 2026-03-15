from __future__ import annotations

from collections.abc import AsyncIterator
from typing import Protocol


class ChatProvider(Protocol):
    async def complete(self, *, system_prompt: str, user_prompt: str) -> str:
        ...

    def stream_complete(
        self, *, system_prompt: str, user_prompt: str
    ) -> AsyncIterator[str]:
        ...
