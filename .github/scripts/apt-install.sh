#!/usr/bin/env bash
#
# The one bounded, retried `apt-get install` for Legion's CI. Every apt call in
# every workflow goes through here.
#
# It exists because unbounded is not a no-op: a stalled apt mirror held the
# install step of `legion-gates.yml` for 2h37m on PR #166 with every later step
# pending. The job budget is 180 minutes, so the run was on course to burn all
# of it and then report a red gate indistinguishable from a code failure -- the
# worst possible signal, because the gates themselves never ran.
#
# The bounds have to tell a hung mirror from a slow one, and getting that wrong
# is its own outage. Two attempts proved it: a 10-minute install cap killed a
# working mirror with only mesa-vulkan-drivers (17.5 MB) left, and a 5-minute
# update cap killed a legitimate update that had fallen back from a failing
# azure.archive mirror to archive.ubuntu.com. Both reported as a red gate that
# looks exactly like a code failure. The budgets below are sized for a degraded
# mirror doing real work, and still fail a true hang well inside the job.
#
# It is one script, and it takes `--gui` rather than living only for GUI
# dependencies, because both of those failures were hand-rolled call sites that
# each got some part of this wrong -- one lost the retry, one had no retry at
# all. A caller that cannot express its install here is a bug in this script.
#
# Usage:
#   apt-install.sh [--gui] [package...]
#
#   --gui   also install the base list every Linux job needs to build and link
#           the egui desktop shell.
#
# A package may be written `name-a|name-b` to mean "whichever of these the
# archive actually has" -- FUSE was renamed between releases, so AppImage
# packaging needs libfuse2 on some and libfuse2t64 on others. It resolves here,
# after `apt-get update`, because the probe needs a populated cache.

set -euo pipefail

# What every Linux job needs to build and link the egui desktop shell.
GUI_PACKAGES=(
  libxkbcommon-dev
  libwayland-dev
  libxrandr-dev
  libxi-dev
  libxcursor-dev
  libx11-dev
  libxcb-render0-dev
  libxcb-shape0-dev
  libxcb-xfixes0-dev
  libgl1-mesa-dev
  libgtk-3-dev
)

# Bound each individual fetch, not only the command as a whole, so one bad
# mirror cannot stall the batch while the rest are healthy.
APT_OPTS=(
  -o Acquire::Retries=3
  -o Acquire::http::Timeout=30
  -o Acquire::https::Timeout=30
)

# Per-attempt caps. `update` gets 10 minutes because a fallback from a failing
# mirror legitimately takes over five; `install` gets 20 because a degraded
# mirror fetching ~100 packages legitimately takes over ten.
UPDATE_TIMEOUT=600
INSTALL_TIMEOUT=1200

WANTED=()
WITH_GUI=0
for arg in "$@"; do
  case "$arg" in
    --gui) WITH_GUI=1 ;;
    -*) echo "unknown flag: $arg" >&2; exit 2 ;;
    *) WANTED+=("$arg") ;;
  esac
done

if [ "$WITH_GUI" = "0" ] && [ "${#WANTED[@]}" = "0" ]; then
  echo "nothing to install: pass --gui, package names, or both" >&2
  exit 2
fi

export DEBIAN_FRONTEND=noninteractive

for attempt in 1 2 3; do
  if timeout "$UPDATE_TIMEOUT" sudo apt-get "${APT_OPTS[@]}" update; then
    break
  fi
  if [ "$attempt" = "3" ]; then
    echo "apt-get update failed or stalled on all 3 attempts; giving up" >&2
    exit 1
  fi
  echo "apt-get update attempt $attempt failed or stalled; retrying" >&2
  sleep 15
done

# Resolve `a|b` alternatives now that the cache is fresh. An alternative with
# no available candidate is fatal: silently dropping it would install a subset
# and fail later in the job, somewhere less obvious.
PACKAGES=()
[ "$WITH_GUI" = "1" ] && PACKAGES+=("${GUI_PACKAGES[@]}")
for wanted in ${WANTED[@]+"${WANTED[@]}"}; do
  case "$wanted" in
    *"|"*)
      resolved=""
      IFS="|" read -r -a candidates <<<"$wanted"
      for candidate in "${candidates[@]}"; do
        if apt-cache show "$candidate" >/dev/null 2>&1; then
          resolved="$candidate"
          break
        fi
      done
      if [ -z "$resolved" ]; then
        echo "none of the alternatives in '$wanted' exist in this archive" >&2
        exit 1
      fi
      echo "resolved '$wanted' to $resolved"
      PACKAGES+=("$resolved")
      ;;
    *)
      PACKAGES+=("$wanted")
      ;;
  esac
done

for attempt in 1 2; do
  if timeout "$INSTALL_TIMEOUT" sudo apt-get "${APT_OPTS[@]}" \
    install -y --no-install-recommends "${PACKAGES[@]}"; then
    exit 0
  fi
  if [ "$attempt" = "2" ]; then
    echo "apt-get install failed or stalled on both attempts; giving up" >&2
    exit 1
  fi
  echo "apt-get install attempt $attempt failed or stalled; retrying" >&2
  sleep 15
done
