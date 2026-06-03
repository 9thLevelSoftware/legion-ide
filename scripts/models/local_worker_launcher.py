"""Dry-run and launch helper for local Legion worker endpoints."""

from __future__ import annotations

import argparse
import ast
import json
from pathlib import Path
import subprocess
import sys
from typing import Any


def parse_workers_config(path: Path) -> list[dict[str, Any]]:
    """Parse the small YAML subset used by config/workers.example.yaml."""

    workers: list[dict[str, Any]] = []
    current: dict[str, Any] | None = None
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or line == "workers:":
            continue
        if line.startswith("- "):
            if current:
                workers.append(current)
            current = {}
            line = line[2:].strip()
        if ":" not in line or current is None:
            continue
        key, value = line.split(":", 1)
        value = value.strip()
        if value.startswith("["):
            current[key.strip()] = ast.literal_eval(value)
        elif value.lower() in {"true", "false"}:
            current[key.strip()] = value.lower() == "true"
        else:
            current[key.strip()] = value.strip('"')
    if current:
        workers.append(current)
    return workers


def validate_worker(worker: dict[str, Any]) -> None:
    """Validate required worker fields."""

    for key in ["id", "kind", "endpoint", "model", "command"]:
        if key not in worker or worker[key] in ("", []):
            raise ValueError(f"worker is missing required field: {key}")
    if not isinstance(worker["command"], list) or not all(
        isinstance(item, str) and item for item in worker["command"]
    ):
        raise ValueError(f"worker {worker.get('id', '<unknown>')} has invalid command")


def main() -> int:
    """Run the worker launcher."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="config/workers.example.yaml")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--pid-dir", default=".legion-workers")
    args = parser.parse_args()

    config_path = Path(args.config)
    workers = parse_workers_config(config_path)
    for worker in workers:
        validate_worker(worker)

    if args.dry_run:
        print(
            json.dumps(
                {
                    "dry_run": True,
                    "config": str(config_path),
                    "worker_count": len(workers),
                    "workers": [
                        {
                            "id": worker["id"],
                            "kind": worker["kind"],
                            "endpoint": worker["endpoint"],
                            "model": worker["model"],
                            "command": worker["command"],
                        }
                        for worker in workers
                    ],
                },
                indent=2,
                sort_keys=True,
            )
        )
        return 0

    pid_dir = Path(args.pid_dir)
    pid_dir.mkdir(parents=True, exist_ok=True)
    launches = []
    for worker in workers:
        process = subprocess.Popen(worker["command"])  # noqa: S603
        launches.append({"id": worker["id"], "pid": process.pid})
    (pid_dir / "workers.json").write_text(
        json.dumps(launches, indent=2, sort_keys=True),
        encoding="utf-8",
    )
    print(json.dumps({"dry_run": False, "launched": launches}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
