#!/usr/bin/env sh
# Starts or dry-runs local OpenAI-compatible Legion worker endpoints.

set -eu

python_bin="${PYTHON:-python3}"
exec "$python_bin" scripts/models/local_worker_launcher.py "$@"
