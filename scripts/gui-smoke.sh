#!/usr/bin/env sh
# GUI desktop smoke wrapper.

set -eu

show_help() {
    cat <<'EOF'
GUI smoke wrapper

Usage:
  sh scripts/gui-smoke.sh [--dry-run] [--beta|--phase-8] [--workspace PATH] [--file PATH]

Modes:
  default    GUI Phase 6 desktop smoke evidence
  --beta     GUI Phase 7 local beta smoke evidence
  --phase-8  GUI phase-8 advanced surface smoke evidence

Phase 8 defaults:
  Evidence: plans/evidence/gui-productization/phase-8-advanced-surface-smoke.md
  Session state: target/gui-phase8-session.json
  Diagnostics export: target/gui-phase8-diagnostics.md
EOF
}

dry_run=false
beta=false
phase8=false
workspace='.'
beta_workspace='target/gui-phase7-beta-workspace'
file=''
duration_ms='1500'
evidence='plans/evidence/gui-productization/phase-6-platform-accessibility-smoke.md'
session_state='target/gui-phase6-session.json'
diagnostics_export='target/gui-phase6-diagnostics.md'

while [ "$#" -gt 0 ]; do
    case "$1" in
        --help|-h)
            show_help
            exit 0
            ;;
        --dry-run)
            dry_run=true
            shift
            ;;
        --beta)
            beta=true
            evidence='plans/evidence/gui-productization/phase-7-local-workflow-smoke.md'
            session_state='target/gui-phase7-session.json'
            diagnostics_export='target/gui-phase7-diagnostics.md'
            shift
            ;;
        --phase-8|--phase8)
            phase8=true
            evidence='plans/evidence/gui-productization/phase-8-advanced-surface-smoke.md'
            session_state='target/gui-phase8-session.json'
            diagnostics_export='target/gui-phase8-diagnostics.md'
            shift
            ;;
        --workspace)
            workspace="$2"
            shift 2
            ;;
        --beta-workspace)
            beta_workspace="$2"
            shift 2
            ;;
        --file)
            file="$2"
            shift 2
            ;;
        --duration-ms)
            duration_ms="$2"
            shift 2
            ;;
        --evidence)
            evidence="$2"
            shift 2
            ;;
        --session-state)
            session_state="$2"
            shift 2
            ;;
        --diagnostics-export)
            diagnostics_export="$2"
            shift 2
            ;;
        *)
            printf '%s\n' "unsupported gui-smoke argument: $1" >&2
            exit 2
            ;;
    esac
done

if [ "$beta" = true ] && [ "$phase8" = true ]; then
    printf '%s\n' 'choose either --beta or --phase-8, not both' >&2
    exit 2
fi

if [ "$beta" = true ]; then
    set -- run -p devil-desktop -- \
        --workspace "$workspace" \
        --evidence "$evidence" \
        --session-state "$session_state" \
        --diagnostics-export "$diagnostics_export" \
        --beta-smoke \
        --beta-workspace "$beta_workspace"
else
    set -- run -p devil-desktop -- \
        --smoke \
        --workspace "$workspace" \
        --duration-ms "$duration_ms" \
        --evidence "$evidence" \
        --session-state "$session_state" \
        --diagnostics-export "$diagnostics_export"
fi

if [ -n "$file" ]; then
    set -- "$@" --file "$file"
fi

if [ "$beta" = true ]; then
    printf '%s\n' 'GUI Phase 7 beta smoke plan'
    printf 'Beta workspace: %s\n' "$beta_workspace"
elif [ "$phase8" = true ]; then
    printf '%s\n' 'GUI Phase 8 advanced surface smoke plan'
else
    printf '%s\n' 'GUI Phase 6 smoke plan'
fi
printf 'Cargo command: cargo'
for arg in "$@"; do
    printf ' %s' "$arg"
done
printf '\nEvidence: %s\n' "$evidence"
printf 'Session state: %s\n' "$session_state"
printf 'Diagnostics export: %s\n' "$diagnostics_export"

if [ "$dry_run" = true ]; then
    printf '%s\n' 'Dry run: smoke command was not executed.'
    exit 0
fi

cargo "$@"
