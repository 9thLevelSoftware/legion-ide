#!/usr/bin/env sh
set -eu

platform=${1:-auto}
if [ "$platform" = "auto" ]; then
  case "$(uname -s)" in
    Darwin*) platform=macos ;;
    Linux*) platform=linux ;;
    MINGW*|MSYS*|CYGWIN*) platform=windows ;;
    *) platform=unknown ;;
  esac
fi

case "$platform" in
  windows)
    process_name=${LEGION_A11Y_PROCESS:-legion-desktop}
    script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
    probe_path="$script_dir/a11y-uia-walk.ps1"
    if command -v cygpath >/dev/null 2>&1; then
      probe_path=$(cygpath -w "$probe_path")
    fi

    powershell_bin=
    for candidate in powershell.exe powershell pwsh.exe pwsh; do
      if command -v "$candidate" >/dev/null 2>&1; then
        powershell_bin=$candidate
        break
      fi
    done
    if [ -z "$powershell_bin" ]; then
      printf '%s\n' 'platform=windows' 'observation=probe-unavailable' \
        'reason=PowerShell was not found on PATH.' >&2
      exit 127
    fi

    printf '%s\n' 'platform=windows' 'observation=live-probe' \
      'probe=scripts/a11y-uia-walk.ps1' "process=$process_name"
    if MSYS_NO_PATHCONV=1 "$powershell_bin" -NoProfile \
      -ExecutionPolicy Bypass -File "$probe_path" -ProcName "$process_name"; then
      exit 0
    else
      probe_status=$?
      exit "$probe_status"
    fi
    ;;
  macos|linux)
    printf '%s\n' "platform=$platform" 'observation=unobserved' 'probe=not-implemented'
    printf '%s\n' 'reason=No committed OS-tree probe or observation exists for this platform.'
    exit 2
    ;;
  *)
    printf '%s\n' "platform=$platform" 'observation=unsupported' 'reason=Unknown host platform.'
    exit 3
    ;;
esac
