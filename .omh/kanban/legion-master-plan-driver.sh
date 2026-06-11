#!/usr/bin/env bash
set -euo pipefail
BOARD="legion-master-plan"
ROOT="/Users/christopherwilloughby/legion-ide"
LOG="$ROOT/.omh/kanban/legion-master-plan-driver.log"
cd "$ROOT"
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) driver start board=$BOARD" >> "$LOG"
while true; do
  ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  stats=$(hermes kanban --board "$BOARD" stats 2>&1 || true)
  echo "[$ts] stats" >> "$LOG"
  echo "$stats" >> "$LOG"
  if echo "$stats" | grep -qE 'todo[[:space:]]+0' \
     && echo "$stats" | grep -qE 'ready[[:space:]]+0' \
     && echo "$stats" | grep -qE 'running[[:space:]]+0' \
     && echo "$stats" | grep -qE 'blocked[[:space:]]+0'; then
    echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) driver complete: no todo/ready/running/blocked cards" >> "$LOG"
    exit 0
  fi
  hermes kanban --board "$BOARD" dispatch --max 1 --json >> "$LOG" 2>&1 || true
  sleep 60
done
