#!/usr/bin/env sh
# Bring up the AT-SPI registry before legion-desktop starts.
#
# AccessKit registers at process start. If this runs after the app is
# already up, desktop.get_child_count() stays 0 (hosted 33636397701).
# Not an Orca session.

set -eu

if [ "$(uname -s)" != "Linux" ]; then
  printf '%s\n' 'platform=linux' 'observation=unobserved' \
    'reason=a11y-atspi-warmup.sh must run on Linux.'
  exit 2
fi

python3 - <<'PY'
import gi
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi
Atspi.init()
desktop = Atspi.get_desktop(0)
print("ATSPI_REGISTRY_READY desktop_children=" + str(desktop.get_child_count()))
PY
