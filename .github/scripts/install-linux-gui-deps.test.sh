#!/usr/bin/env bash
# Prove the shared installer's package-alternative handling without a Debian
# box, by putting fakes for sudo/apt-get/apt-cache/timeout ahead of it on PATH.
#
# This exists because the alternative syntax is real logic in CI plumbing, and
# CI plumbing is only exercised when it breaks. The empty-extras case matters
# most: the script runs under `set -u`, where an unguarded empty array
# expansion is an error rather than nothing.
#
# Run by the Linux gate job; runnable by hand as:
#   bash .github/scripts/install-linux-gui-deps.test.sh .github/scripts/install-linux-gui-deps.sh
set -uo pipefail

SCRIPT="${1:-$(dirname "$0")/install-linux-gui-deps.sh}"
STUBS="$(mktemp -d)"
trap 'rm -rf "$STUBS"' EXIT

cat >"$STUBS/sudo" <<'EOF'
#!/usr/bin/env bash
exec "$@"
EOF
cat >"$STUBS/timeout" <<'EOF'
#!/usr/bin/env bash
shift
exec "$@"
EOF
cat >"$STUBS/apt-get" <<'EOF'
#!/usr/bin/env bash
if [ "${1:-}" = "update" ] || [ "$*" = "update" ]; then
  echo "STUB update ok"
  exit 0
fi
for arg in "$@"; do
  case "$arg" in
    -o|update|install|-y|--no-install-recommends) continue ;;
    Acquire::*) continue ;;
    *) echo "PKG $arg" ;;
  esac
done
exit 0
EOF
cat >"$STUBS/apt-cache" <<'EOF'
#!/usr/bin/env bash
# Only the "t64" name exists in this fake archive.
[ "$2" = "libfuse2t64" ] && exit 0
exit 1
EOF
chmod +x "$STUBS"/*
export PATH="$STUBS:$PATH"

echo "--- case 1: alternative resolves to the name that exists ---"
out=$(bash "$SCRIPT" 'libfuse2|libfuse2t64' 2>&1)
echo "$out" | grep -q "resolved 'libfuse2|libfuse2t64' to libfuse2t64" \
  && echo "PASS resolved to libfuse2t64" || { echo "FAIL: $out"; exit 1; }
echo "$out" | grep -q "^PKG libfuse2t64$" \
  && echo "PASS installed the resolved name" || { echo "FAIL: $out"; exit 1; }
echo "$out" | grep -q "^PKG libgtk-3-dev$" \
  && echo "PASS base packages still installed" || { echo "FAIL: $out"; exit 1; }

echo "--- case 2: no alternative exists -> fatal, not a silent subset ---"
out=$(bash "$SCRIPT" 'nosuchpkg|alsomissing' 2>&1); rc=$?
[ "$rc" -ne 0 ] && echo "PASS exits non-zero ($rc)" || { echo "FAIL: exited 0"; exit 1; }
echo "$out" | grep -q "none of the alternatives" \
  && echo "PASS names the failure" || { echo "FAIL: $out"; exit 1; }

echo "--- case 3: plain extras and no extras at all ---"
out=$(bash "$SCRIPT" mesa-vulkan-drivers 2>&1)
echo "$out" | grep -q "^PKG mesa-vulkan-drivers$" \
  && echo "PASS plain extra passed through" || { echo "FAIL: $out"; exit 1; }
out=$(bash "$SCRIPT" 2>&1)
echo "$out" | grep -q "^PKG libgtk-3-dev$" \
  && echo "PASS no-extras call works under set -u" || { echo "FAIL: $out"; exit 1; }

echo "ALL CASES PASS"
