#!/usr/bin/env bash
# Prove apt-install.sh's argument handling and package-alternative resolution
# without a Debian box, by putting fakes for sudo/apt-get/apt-cache/timeout
# ahead of it on PATH.
#
# This exists because the script is real logic in CI plumbing, and CI plumbing
# is only exercised when it breaks. Two of its cases are the kind that pass
# review and fail in production: an empty package array under `set -u`, where an
# unguarded expansion is an error rather than nothing, and `--gui` with no
# extras, which is how most callers invoke it.
#
# Run by the Linux gate job; runnable by hand as:
#   bash .github/scripts/apt-install.test.sh
set -uo pipefail

SCRIPT="${1:-$(dirname "$0")/apt-install.sh}"
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

fail() { echo "FAIL: $1"; echo "--- output ---"; echo "$2"; exit 1; }

echo "--- alternatives resolve to the name the archive has ---"
out=$(bash "$SCRIPT" --gui 'libfuse2|libfuse2t64' 2>&1)
echo "$out" | grep -q "resolved 'libfuse2|libfuse2t64' to libfuse2t64" \
  || fail "did not resolve the alternative" "$out"
echo "$out" | grep -q "^PKG libfuse2t64$" || fail "did not install resolved name" "$out"
echo "$out" | grep -q "^PKG libgtk-3-dev$" || fail "--gui did not add the base list" "$out"
echo "PASS"

echo "--- no candidate is fatal, not a silent subset ---"
out=$(bash "$SCRIPT" --gui 'nosuchpkg|alsomissing' 2>&1); rc=$?
[ "$rc" -ne 0 ] || fail "exited 0 with no candidate" "$out"
echo "$out" | grep -q "none of the alternatives" || fail "did not name the failure" "$out"
echo "PASS"

echo "--- --gui with no extras (the common call) ---"
out=$(bash "$SCRIPT" --gui 2>&1)
echo "$out" | grep -q "^PKG libgtk-3-dev$" || fail "--gui alone installed nothing" "$out"
echo "$out" | grep -q "^PKG lldb$" && fail "installed something never asked for" "$out"
echo "PASS"

echo "--- packages without --gui skip the base list ---"
out=$(bash "$SCRIPT" lldb 2>&1)
echo "$out" | grep -q "^PKG lldb$" || fail "did not install the named package" "$out"
echo "$out" | grep -q "^PKG libgtk-3-dev$" && fail "pulled the GUI list in without --gui" "$out"
echo "PASS"

echo "--- --gui plus extras ---"
out=$(bash "$SCRIPT" --gui mesa-vulkan-drivers 2>&1)
echo "$out" | grep -q "^PKG mesa-vulkan-drivers$" || fail "dropped the extra" "$out"
echo "$out" | grep -q "^PKG libgtk-3-dev$" || fail "dropped the base list" "$out"
echo "PASS"

echo "--- an empty invocation is refused rather than installing nothing ---"
out=$(bash "$SCRIPT" 2>&1); rc=$?
[ "$rc" -eq 2 ] || fail "expected exit 2 for an empty invocation, got $rc" "$out"
echo "PASS"

echo "--- an unknown flag is refused rather than treated as a package ---"
out=$(bash "$SCRIPT" --gui --nope 2>&1); rc=$?
[ "$rc" -eq 2 ] || fail "expected exit 2 for an unknown flag, got $rc" "$out"
echo "$out" | grep -q "^PKG --nope$" && fail "passed a flag to apt as a package" "$out"
echo "PASS"

echo "ALL CASES PASS"
