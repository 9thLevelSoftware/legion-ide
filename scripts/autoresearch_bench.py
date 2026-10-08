#!/usr/bin/env python3
"""Execute the frozen governed cassette workload, then validate measured scores."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "scripts/autoresearch_bench_inputs.json"
OUTPUT = ROOT / "target/autoresearch-bench"
CORPUS = "evals/legion-bench/tasks"
CASSETTES = "evals/legion-bench/recorded"
GENERATED = ("legion_bench_report.toml", "live_run_input.toml", "live_run_results.toml")
EXCLUDED = {"target", ".git", "__pycache__"}
SCORING_MODE = "recorded_replay_execution"
PROVIDER = "recorded:qwen2.5-coder:14b@governed"
COMMAND = [
    "cargo", "run", "--offline", "--locked", "-p", "xtask", "--",
    "legion-bench", "--mode", "recorded", "--corpus", CORPUS,
    "--cassettes", CASSETTES, "--out", "target/autoresearch-bench",
]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def integer(value, label, maximum=2**32 - 1):
    require(type(value) is int and 0 <= value <= maximum, f"invalid {label}: {value!r}")
    return value


def boolean(value, label):
    require(type(value) is bool, f"invalid {label}: {value!r}")
    return value


def load_toml(path):
    with path.open("rb") as stream:
        return tomllib.load(stream)


def input_path(relative):
    path = Path(relative)
    require(not path.is_absolute() and ".." not in path.parts, f"unsafe input path: {relative}")
    result = ROOT / path
    require(result.resolve().is_relative_to(ROOT), f"input escapes repository: {relative}")
    require(not result.is_symlink(), f"symlink input: {relative}")
    return result


def normalized_hash(path):
    data = path.read_bytes()
    try:
        data = data.decode("utf-8").replace("\r\n", "\n").encode("utf-8")
    except UnicodeDecodeError:
        pass
    return hashlib.sha256(data).hexdigest()


def input_files(manifest):
    files = set(manifest["fixed_files"])
    for relative in manifest["directory_roots"]:
        directory = input_path(relative)
        require(directory.is_dir(), f"missing input directory: {relative}")
        for current, directories, names in os.walk(directory, followlinks=False):
            directories[:] = sorted(name for name in directories if name not in EXCLUDED)
            for name in directories + names:
                require(not (Path(current) / name).is_symlink(), f"symlink input: {current}/{name}")
            for name in names:
                if name not in EXCLUDED:
                    files.add((Path(current) / name).relative_to(ROOT).as_posix())
    return files


def verify_inputs(manifest):
    require(manifest["schema_version"] == 1, "unsupported input manifest schema")
    expected = manifest["sha256"]
    actual = input_files(manifest)
    require(actual == set(expected),
            f"input file set changed: added={sorted(actual - set(expected))} "
            f"deleted={sorted(set(expected) - actual)}")
    for relative in sorted(actual):
        require(normalized_hash(input_path(relative)) == expected[relative],
                f"frozen input changed: {relative}")


def indexed(rows, key, label):
    require(type(rows) is list, f"invalid {label} rows")
    result = {}
    for row in rows:
        task_id = key(row)
        require(type(task_id) is str and task_id not in result, f"duplicate/invalid {label} id: {task_id!r}")
        result[task_id] = row
    return result


def validate_report(manifest):
    report = load_toml(OUTPUT / GENERATED[0])
    raw_output = load_toml(OUTPUT / GENERATED[2])
    run_input = load_toml(OUTPUT / GENERATED[1])
    require(report["schema_version"] == 3, "report schema must be 3")
    require(report["scoring_mode"] == SCORING_MODE, "report is not executed recorded replay")
    require(report["mode"] == "recorded_offline", "report mode is not recorded_offline")
    require(report["provider_profile"] == PROVIDER, "report provider/arm changed")
    require(report["suite_fingerprint"] == manifest["suite_fingerprint"], "suite fingerprint changed")
    require(raw_output["schema_version"] == 1 and run_input["schema_version"] == 1,
            "runner interchange schema changed")
    require(run_input["provider_mode"] == "replay" and run_input["endpoint"] == "replay://cassettes",
            "runner did not use offline replay transport")
    require(Path(run_input["cassette_dir"]).resolve() == (ROOT / CASSETTES).resolve(),
            "runner used a different cassette directory")
    corpus = indexed([load_toml(path) for path in sorted((ROOT / CORPUS).glob("*.toml"))],
                     lambda row: row["id"], "corpus")
    executed = set(manifest["executed_ids"])
    holdouts = set(manifest["holdout_ids"])
    require(len(corpus) == 25 and len(executed) == 20 and len(holdouts) == 5,
            "expected 25 corpus tasks, 20 executed tasks, 5 holdouts")
    require(not executed & holdouts and set(corpus) == executed | holdouts, "corpus membership changed")
    require({task_id for task_id, row in corpus.items() if row.get("holdout", False)} == holdouts,
            "holdout exclusion changed")
    require(sorted({corpus[task_id]["fixture_repo"] for task_id in executed}) == manifest["fixture_roots"],
            "executed fixture roots changed")
    results = indexed(report["tasks"], lambda row: row["task"]["id"], "report")
    raw = indexed(raw_output["results"], lambda row: row["id"], "runner")
    planned = indexed(run_input["tasks"], lambda row: row["id"], "runner input")
    require(set(results) == set(corpus), "report task set changed")
    require(set(raw) == executed and set(planned) == executed, "runner task set changed or results missing")
    passed = verification_passes = drift = score_total = 0
    for task_id, row in results.items():
        definition = corpus[task_id]
        score = row["score"]
        value = integer(score["score"], f"{task_id} score", 100)
        tests_passed = boolean(score["tests_passed"], f"{task_id} tests_passed")
        for field in ("diff_files", "turns", "cost_cents"):
            integer(score[field], f"{task_id} {field}")
        require(row["task"]["fixture_repo"] == definition["fixture_repo"] and
                row["task"]["gate_budget"] == definition["gate_budget"], f"{task_id} task definition changed")
        if task_id in holdouts:
            require(score["status"] == "skipped" and "live" not in score and value == 0 and
                    not tests_passed and all(score[field] == 0 for field in ("diff_files", "turns", "cost_cents")),
                    f"{task_id} holdout was graded")
            continue
        require(score["status"] in ("passed", "failed"), f"{task_id} graded status invalid")
        live = score.get("live")
        require(type(live) is dict, f"{task_id} missing execution metrics")
        measured = raw[task_id]
        require(measured["cassette_model"] == "qwen2.5-coder:14b" and measured["cassette_arm"] == "governed",
                f"{task_id} cassette provenance changed")
        tape = json.loads((ROOT / CASSETTES / f"{task_id}.json").read_text(encoding="utf-8"))
        exchanges = integer(live["cassette_exchanges"], f"{task_id} cassette_exchanges")
        task_drift = integer(live["cassette_drift"], f"{task_id} cassette_drift")
        require(0 < exchanges <= len(tape["exchanges"]) and task_drift <= exchanges,
                f"{task_id} did not replay valid cassette exchanges")
        boolean(live["task_success"], f"{task_id} task_success")
        for field in ("tool_calls", "duplicate_tool_calls", "retries"):
            integer(live[field], f"{task_id} {field}")
        require(live["duplicate_tool_calls"] <= live["tool_calls"] and live["retries"] <= live["tool_calls"],
                f"{task_id} inconsistent tool metrics")
        for field in ("wall_ms", "context_tokens", "generation_tokens"):
            if field in live:
                integer(live[field], f"{task_id} {field}", 2**64 - 1)
        for field, metric in live.items():
            require(measured.get(field) == metric, f"{task_id} raw/report {field} mismatch")
        for field in ("tests_passed", "diff_files", "turns"):
            require(measured[field] == score[field], f"{task_id} raw/report {field} mismatch")
        require(score["cost_cents"] == 0, f"{task_id} recorded replay has billing cost")
        budget = definition["gate_budget"]
        success = (live["task_success"] and (not budget["require_tests_pass"] or tests_passed)
                   and score["diff_files"] <= budget["max_diff_files"]
                   and score["turns"] <= budget["max_turns"])
        require((score["status"] == "passed") == success, f"{task_id} pass status inconsistent")
        weights = definition.get("scoring", {})
        expected_score = max(0, 100 - min(score["diff_files"], budget["max_diff_files"]) * weights.get("diff_file_penalty", 4))
        expected_score = max(0, expected_score - min(score["turns"], budget["max_turns"]) * weights.get("turn_penalty", 3))
        if not success:
            expected_score = max(0, expected_score - weights.get("fail_penalty", 40))
        require(value == expected_score, f"{task_id} score inconsistent with measured execution")
        if tests_passed:
            require(measured.get("verification_exit") == definition["verification"]["expected_exit"],
                    f"{task_id} verification pass has no matching exit")
        passed += score["status"] == "passed"
        verification_passes += tests_passed
        drift += task_drift
        score_total += value
    average = score_total // len(executed)
    expected_summary = dict(total=25, passed=passed, failed=20 - passed, skipped=5, regressed=0, average_score=average)
    for field, value in expected_summary.items():
        require(integer(report["summary"][field], f"summary {field}") == value, f"summary {field} mismatch")
    return average, passed, verification_passes, drift


def main():
    require(OUTPUT.resolve() == ROOT / "target/autoresearch-bench", "output directory redirects outside fixed target path")
    OUTPUT.mkdir(parents=True, exist_ok=True)
    for filename in GENERATED:
        path = OUTPUT / filename
        require(not path.is_symlink(), f"generated report is a symlink: {path}")
        path.unlink(missing_ok=True)
    require(len(sys.argv) == 1, "benchmark harness takes no arguments")
    require(sys.version_info >= (3, 11), "Python >=3.11 is required")
    environment = os.environ.copy()
    for key in ("LEGION_BENCH_ENDPOINT", "LEGION_BENCH_MODEL", "LEGION_BENCH_API_KEY", "LEGION_BENCH_KEEP_CHECKOUTS"):
        environment.pop(key, None)
    environment.update(CARGO_NET_OFFLINE="true", CARGO_BUILD_JOBS="1", LEGION_AI_GOVERNORS="on",
                       PYTHONHASHSEED="0", PYTHONDONTWRITEBYTECODE="1")
    for executable in ("cargo", "git", "python", "node"):
        require(shutil.which(executable, path=environment.get("PATH")) is not None, f"missing prerequisite: {executable}")
        version = subprocess.run([executable, "--version"], cwd=ROOT, env=environment,
                                 check=True, capture_output=True, text=True)
        print(f"ASI runtime_{executable}={version.stdout.strip() or version.stderr.strip()}", flush=True)
    subprocess.run(["python", "-c", "import sys, tomllib; sys.exit(0 if sys.version_info >= (3, 11) else 1)"],
                   cwd=ROOT, env=environment, check=True)
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    verify_inputs(manifest)
    subprocess.run(COMMAND, cwd=ROOT, env=environment, check=True)
    verify_inputs(manifest)
    average, passed, verification_passes, drift = validate_report(manifest)
    print("ASI corpus_tasks=25 executed_tasks=20 holdout_tasks=5")
    print(f"METRIC recorded_task_score={average}")
    print(f"METRIC passed_tasks={passed}")
    print(f"METRIC verification_passes={verification_passes}")
    print(f"METRIC cassette_drift={drift}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        print(f"autoresearch failed: {error}", file=sys.stderr)
        sys.exit(1)
