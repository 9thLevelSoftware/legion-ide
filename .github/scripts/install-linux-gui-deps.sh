#!/usr/bin/env bash
#
# Install the Linux GUI build dependencies Legion's workflows need, with
# bounded fetches and retries.
#
# This exists because unbounded is not a no-op: a stalled apt mirror held the
# install step of `legion-gates.yml` for 2h37m on PR #166 with every later step
# pending. The job budget is 180 minutes, so the run was on course to burn all
# of it and then report a red gate indistinguishable from a code failure -- the
# worst possible signal, because the gates themselves never ran.
#
# The bounds distinguish a hung mirror from a slow one, which the first attempt
# at this did not: a 10-minute install cap killed a degraded-but-working mirror
# at exit 124 with ~100 packages fetched and only mesa-vulkan-drivers (17.5 MB)
# left. Too tight a bound is its own outage. What is here fails a true hang in
# well under a third of a 180-minute job while leaving a slow mirror room to
# finish, and apt caches partial downloads under /var/cache/apt/archives, so a
# retry resumes rather than restarting the fetch.
#
# It is one script rather than a block pasted into five workflows because
# copy-paste is how all eight unbounded call sites came to exist. A new
# workflow that calls this gets the bounding for free.
#
# Usage: .github/scripts/install-linux-gui-deps.sh [extra-package...]
#
# The base list is what every Linux job needs to build and link the egui
# desktop shell. Callers add only what is theirs: `mesa-vulkan-drivers` for
# jobs that render, `libdbus-1-dev` for the DAP dogfood, the packaging set for
# AppImage builds.
#
# An extra may be written `name-a|name-b` to mean "whichever of these the
# archive actually has" -- FUSE was renamed between releases, so AppImage
# packaging needs libfuse2 on some and libfuse2t64 on others. Resolving it
# here rather than in the caller is not tidiness: the probe needs a populated
# apt cache, and a caller doing it by hand also had to hand-roll the retry,
# which is how the release job ended up with a bounded install that could not
# recover from the stall it was bounded against.

set -euo pipefail

BASE_PACKAGES=(
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

export DEBIAN_FRONTEND=noninteractive

for attempt in 1 2 3; do
  if timeout 300 sudo apt-get "${APT_OPTS[@]}" update; then
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
EXTRA_PACKAGES=()
for extra in "$@"; do
  case "$extra" in
    *"|"*)
      resolved=""
      IFS="|" read -r -a candidates <<<"$extra"
      for candidate in "${candidates[@]}"; do
        if apt-cache show "$candidate" >/dev/null 2>&1; then
          resolved="$candidate"
          break
        fi
      done
      if [ -z "$resolved" ]; then
        echo "none of the alternatives in '$extra' exist in this archive" >&2
        exit 1
      fi
      echo "resolved '$extra' to $resolved"
      EXTRA_PACKAGES+=("$resolved")
      ;;
    *)
      EXTRA_PACKAGES+=("$extra")
      ;;
  esac
done

for attempt in 1 2; do
  if timeout 1200 sudo apt-get "${APT_OPTS[@]}" install -y --no-install-recommends \
    "${BASE_PACKAGES[@]}" ${EXTRA_PACKAGES[@]+"${EXTRA_PACKAGES[@]}"}; then
    exit 0
  fi
  if [ "$attempt" = "2" ]; then
    echo "apt-get install failed or stalled on both attempts; giving up" >&2
    exit 1
  fi
  echo "apt-get install attempt $attempt failed or stalled; retrying" >&2
  sleep 15
done
