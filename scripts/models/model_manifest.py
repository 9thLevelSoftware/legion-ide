"""Phase 8 model roster used by dry-run helpers."""

from __future__ import annotations

from dataclasses import dataclass
import json


@dataclass(frozen=True)
class ModelSpec:
    """A model acquisition target."""

    model_id: str
    role: str
    local_worker_id: str
    notes: str


MODEL_ROSTER = [
    ModelSpec(
        "Qwen/Qwen2.5-Coder-1.5B-Instruct",
        "docs-summarizer",
        "qwen25-coder-1_5b",
        "small local documentation summarizer candidate",
    ),
    ModelSpec(
        "Qwen/Qwen2.5-Coder-3B-Instruct",
        "rust-compiler-fixer",
        "qwen25-coder-3b",
        "small local Rust compiler-fix candidate",
    ),
    ModelSpec(
        "Qwen/Qwen2.5-Coder-7B-Instruct",
        "reviewer",
        "qwen25-coder-7b",
        "review and patch-quality candidate",
    ),
    ModelSpec(
        "Qwen/Qwen2.5-Coder-14B-Instruct",
        "heavy-reviewer",
        "qwen25-coder-14b",
        "larger local or cloud escalation candidate",
    ),
    ModelSpec(
        "bigcode/starcoder2-3b",
        "test-writer-baseline",
        "starcoder2-3b",
        "baseline test-writing specialist candidate",
    ),
    ModelSpec(
        "deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct",
        "remote-escalation-baseline",
        "deepseek-coder-v2-lite",
        "larger remote or dedicated GPU worker candidate",
    ),
]


def roster_as_dicts() -> list[dict[str, str]]:
    """Return the roster as JSON-safe dictionaries."""

    return [spec.__dict__.copy() for spec in MODEL_ROSTER]


def main() -> None:
    """Print the model roster as JSON."""

    print(json.dumps({"models": roster_as_dicts()}, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
