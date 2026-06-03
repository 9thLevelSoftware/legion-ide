#!/usr/bin/env sh
# Phase 8 model acquisition helper for Legion dry-run and operator use.

set -eu

dry_run=false
output_dir="models"

usage() {
    cat <<'EOF'
Usage: scripts/models/download-models.sh [--dry-run] [--output-dir DIR]

Downloads the Phase 8 base-model roster with huggingface-cli when not in
dry-run mode. Dry-run mode prints the exact model IDs without network access.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run)
            dry_run=true
            shift
            ;;
        --output-dir)
            output_dir="${2:?missing output dir}"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

model_roster='Qwen/Qwen2.5-Coder-1.5B-Instruct|docs-summarizer
Qwen/Qwen2.5-Coder-3B-Instruct|rust-compiler-fixer
Qwen/Qwen2.5-Coder-7B-Instruct|reviewer
Qwen/Qwen2.5-Coder-14B-Instruct|heavy-reviewer
bigcode/starcoder2-3b|test-writer-baseline
deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct|remote-escalation-baseline'

printf '%s\n' "Legion Phase 8 model acquisition roster"
printf '%s\n' "output_dir=$output_dir"

if [ "$dry_run" = true ]; then
    printf '%s\n' "dry_run=true"
    printf '%s\n' "$model_roster" | while IFS='|' read -r model_id role; do
        [ -n "$model_id" ] || continue
        printf '%s\n' "would_download model_id=$model_id role=$role"
    done
    printf '%s\n' "network=disabled"
    exit 0
fi

if ! command -v huggingface-cli >/dev/null 2>&1; then
    echo "huggingface-cli is required outside --dry-run mode" >&2
    exit 127
fi

mkdir -p "$output_dir"
printf '%s\n' "$model_roster" | while IFS='|' read -r model_id role; do
    [ -n "$model_id" ] || continue
    safe_name=$(printf '%s' "$model_id" | tr '/:' '__')
    target_dir="$output_dir/$safe_name"
    printf '%s\n' "downloading model_id=$model_id role=$role target=$target_dir"
    huggingface-cli download "$model_id" \
        --local-dir "$target_dir" \
        --local-dir-use-symlinks False
done

printf '%s\n' "download_complete"
