#!/usr/bin/env sh
# Linux AT-SPI walk of a running legion-desktop window.
#
# WHY THIS EXISTS
# ---------------
# Windows has scripts/a11y-uia-walk.ps1. Linux had no committed OS-tree probe
# (GAP-05.4). This script is the Linux leg.
#
# SCOPE: AT-SPI tree only. It is not an Orca session (GAP-05.4 still needs
# Orca notes). macOS is scripts/a11y-ax-walk.sh.
#
# USAGE
# -----
#   1. Start a live window (xvfb + legion-desktop --smoke).
#   2. LEGION_A11Y_PROCESS=legion-desktop ./scripts/a11y-atspi-walk.sh
#
# Requires: python3, PyGObject, Atspi 2.0 (gir1.2-atspi-2.0).
# Exit codes: 0 walked; 2 not Linux; 3 bindings missing; 4 process not
# running; 5 no accessible window.

set -eu

if [ "$(uname -s)" != "Linux" ]; then
  printf '%s\n' 'platform=linux' 'observation=unobserved' \
    'reason=a11y-atspi-walk.sh must run on Linux.'
  exit 2
fi

process_name=${LEGION_A11Y_PROCESS:-legion-desktop}

if ! python3 -c "import gi; gi.require_version('Atspi','2.0'); from gi.repository import Atspi" \
  >/dev/null 2>&1; then
  printf '%s\n' 'ATSPI_BINDINGS_MISSING: python3 gi Atspi 2.0'
  exit 3
fi

PROCESS_NAME=$process_name python3 - <<'PY'
import os
import sys

import gi
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi

name = os.environ.get("PROCESS_NAME", "legion-desktop")
Atspi.init()
desktop = Atspi.get_desktop(0)
total = 0
limit = 400

def walk(acc, depth):
    global total
    if depth > 5 or total >= limit:
        return
    try:
        n = acc.get_child_count()
    except Exception:
        return
    for i in range(n):
        if total >= limit:
            return
        try:
            child = acc.get_child_at_index(i)
        except Exception:
            continue
        if child is None:
            continue
        total += 1
        role = ""
        title = ""
        try:
            role = child.get_role_name() or ""
        except Exception:
            pass
        try:
            title = child.get_name() or ""
        except Exception:
            pass
        print(f"{'  ' * depth}[{depth}] {role} name='{title}'")
        walk(child, depth + 1)

matched = False
seen = []
for i in range(desktop.get_child_count()):
    app = desktop.get_child_at_index(i)
    if app is None:
        continue
    app_name = app.get_name() or ""
    seen.append(app_name)
    if name.lower() not in app_name.lower() and "legion" not in app_name.lower():
        continue
    matched = True
    print(f"APP name='{app_name}'")
    walk(app, 1)

if not matched:
    print("ATSPI_APPS:")
    for app_name in seen:
        print(f"  name='{app_name}'")
    print(f"PROCESS_NOT_FOUND: {name}")
    sys.exit(4)
if total == 0:
    print("WINDOW_NOT_FOUND: AT-SPI published no descendants")
    sys.exit(5)
print(f"DESCENDANTS_ENUMERATED: {total}")
print("ATSPI_WALK_OK")
PY
