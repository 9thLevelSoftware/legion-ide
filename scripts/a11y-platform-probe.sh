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
    printf '%s\n' 'platform=windows' 'observation=delegated' 'probe=scripts/a11y-uia-walk.ps1'
    printf '%s\n' 'run: powershell -NoProfile -ExecutionPolicy Bypass -File scripts/a11y-uia-walk.ps1'
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
