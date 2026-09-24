#!/usr/bin/env sh
# macOS Accessibility (AX) walk of a running legion-desktop window.
#
# WHY THIS EXISTS
# ---------------
# Windows has scripts/a11y-uia-walk.ps1. macOS had no committed OS-tree probe
# (GAP-05.3). The 2026-08-16 AX dump in WS18-T2 was an external probe whose
# source was never committed. This script is the macOS leg, so an observation
# can be repeated.
#
# SCOPE: macOS AX tree only. It is not a VoiceOver session (GAP-05.3 still
# needs VoiceOver notes). Linux is scripts/a11y-atspi-walk.sh.
#
# USAGE
# -----
#   1. Start a live window (legion-desktop --smoke or --windowed-e2e).
#   2. LEGION_A11Y_PROCESS=legion-desktop ./scripts/a11y-ax-walk.sh
#
# Exit codes: 0 walked; 2 not macOS; 4 process not running; 5 no AX window
# or permission denied.

set -eu

if [ "$(uname -s)" != "Darwin" ]; then
  printf '%s\n' 'platform=macos' 'observation=unobserved' \
    'reason=a11y-ax-walk.sh must run on macOS.'
  exit 2
fi

process_name=${LEGION_A11Y_PROCESS:-legion-desktop}

if ! pgrep -x "$process_name" >/dev/null 2>&1; then
  printf '%s\n' "PROCESS_NOT_FOUND: $process_name"
  exit 4
fi

printf '%s\n' "PROCESS_FOUND: $process_name"

# System Events is the committed, repeatable AX walk. It is not VoiceOver.
# osascript `log` is not stdout; return one text blob instead.
# Hosted osascript rejected `return dumpState` (33636397701 975:983,
# 33637474631 1228:1236). Keep this script to one `on run` handler and
# never `return` a record.
# `set -e` would skip the status handler if osascript fails, so capture it.
set +e
osascript - "$process_name" <<'APPLESCRIPT'
on run argv
  set procName to item 1 of argv
  set outText to ""
  set total to 0
  tell application "System Events"
    if not (exists process procName) then
      error "PROCESS_NOT_FOUND: " & procName
    end if
    tell process procName
      set winCount to count of windows
      set outText to outText & "WINDOW_COUNT=" & (winCount as text) & linefeed
      if winCount is 0 then
        error "WINDOW_NOT_FOUND"
      end if
      repeat with w in windows
        set total to total + 1
        set wtitle to ""
        try
          set wtitle to name of w as text
        end try
        set outText to outText & "[1] window name='" & wtitle & "'" & linefeed
        try
          set kids to UI elements of w
          repeat with kid in kids
            if total > 399 then
              exit repeat
            end if
            set total to total + 1
            set roleName to ""
            set titleName to ""
            try
              set roleName to role of kid as text
            end try
            try
              set titleName to name of kid as text
            end try
            set outText to outText & "  [2] " & roleName & " name='" & titleName & "'" & linefeed
          end repeat
        end try
      end repeat
      set outText to outText & "DESCENDANTS_ENUMERATED: " & (total as text) & linefeed
    end tell
  end tell
  return outText & "AX_WALK_OK" & linefeed
end run
APPLESCRIPT
status=$?
set -e
if [ "$status" -ne 0 ]; then
  printf '%s\n' "AX_WALK_FAILED: osascript exit $status (often TCC / Accessibility permission)"
  exit 5
fi
exit 0
