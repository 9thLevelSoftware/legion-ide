#!/usr/bin/env bash
set -euo pipefail

if (( $# != 0 )); then
    printf '%s\n' 'autoresearch.sh takes no arguments; its recorded workload is fixed.' >&2
    exit 2
fi
repo_root="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
export PYTHONHASHSEED=0
export PYTHONDONTWRITEBYTECODE=1
if ! command -v cargo >/dev/null 2>&1 && command -v cargo.exe >/dev/null 2>&1; then
    if ! command -v wslpath >/dev/null 2>&1; then
        printf '%s\n' 'Native Cargo requires the WSL bridge, but wslpath is unavailable.' >&2
        exit 1
    fi
    if ! command -v python.exe >/dev/null 2>&1 ||
        ! python.exe -B -c 'import sys, tomllib; sys.exit(0 if sys.version_info >= (3, 11) else 1)' >/dev/null 2>&1; then
        printf '%s\n' 'WSL bridge requires Windows Python >=3.11 with tomllib.' >&2
        exit 1
    fi
    helper_path="$(wslpath -w "$repo_root/scripts/autoresearch_bench.py")"
    export WSLENV="${WSLENV:+$WSLENV:}PYTHONHASHSEED:PYTHONDONTWRITEBYTECODE"
    exec python.exe -B "$helper_path"
fi
for candidate in python3 python; do
    if command -v "$candidate" >/dev/null 2>&1 &&
        "$candidate" -c 'import sys, tomllib; sys.exit(0 if sys.version_info >= (3, 11) else 1)' >/dev/null 2>&1; then
        exec "$candidate" "$repo_root/scripts/autoresearch_bench.py"
    fi
done
printf '%s\n' 'autoresearch requires Python >=3.11 with standard-library tomllib.' >&2
exit 1
