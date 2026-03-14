from __future__ import annotations

from collections.abc import Mapping, Sequence
from functools import lru_cache

from tradstry_agents.config import Settings
from tradstry_agents.schemas import AgentRoute, MemoryKind


class PromptLibrary:
    def __init__(self, settings: Settings):
        self._root = settings.prompt_root

    def compose_system_prompt(self, route: AgentRoute | str) -> str:
        normalized_route = route.value if isinstance(route, AgentRoute) else route
        sections = [
            self._read("shared/base_system.txt"),
            self._read("shared/tool_rules.txt"),
            self._read("shared/tool_examples.txt"),
            self._read(f"routes/{normalized_route}.txt"),
        ]
        return "\n\n".join(section.strip() for section in sections if section.strip())

    def format_memory_context(self, memories: Sequence[Mapping[str, object]]) -> str:
        if not memories:
            return "No relevant long-term memory was retrieved."

        lines: list[str] = []
        for memory in sorted(memories, key=self._memory_sort_key):
            raw_kind = memory.get("kind", "memory")
            if isinstance(raw_kind, MemoryKind):
                kind = raw_kind.value.replace("_", " ")
            else:
                kind = str(raw_kind).replace("_", " ")
            abstract = str(memory.get("abstract", "")).strip()
            age_days = memory.get("age_days")
            confidence = memory.get("confidence")
            importance = memory.get("importance")
            details: list[str] = []
            if isinstance(age_days, (int, float)):
                details.append(f"{age_days:.0f}d old")
            if isinstance(confidence, (int, float)):
                details.append(f"confidence {confidence:.2f}")
            if isinstance(importance, (int, float)):
                details.append(f"importance {importance:.2f}")
            suffix = f" ({', '.join(details)})" if details else ""
            lines.append(f"- [{kind}] {abstract}{suffix}")
        return "\n".join(lines)

    @lru_cache(maxsize=16)
    def _read(self, relative_path: str) -> str:
        return (self._root / relative_path).read_text(encoding="utf-8")

    @staticmethod
    def _memory_sort_key(memory: Mapping[str, object]) -> tuple[int, float]:
        kind = memory.get("kind")
        if isinstance(kind, MemoryKind):
            normalized_kind = kind.value
        else:
            normalized_kind = str(kind)
        priority = {
            MemoryKind.PREFERENCE.value: 0,
            MemoryKind.GOAL.value: 1,
            MemoryKind.PATTERN.value: 2,
            MemoryKind.EVENT.value: 3,
        }.get(normalized_kind, 99)
        age_days = memory.get("age_days")
        normalized_age = float(age_days) if isinstance(age_days, (int, float)) else 9999.0
        return (priority, normalized_age)
