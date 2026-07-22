#!/usr/bin/env bash
# WS-A-D Phase 4 D1: build portable unsigned-beta preview bundle (Unix).
#
# Produces:
#   target/preview/<os>-<arch>/legion-desktop
#   target/preview/<os>-<arch>/UNSIGNED-BETA.toml
#   target/preview/<os>-<arch>/package-manifest.txt
#   target/preview/legion-desktop-preview-<os>-<arch>.tar.gz

set -euo pipefail

RELEASE=0
OUT_ROOT="target/preview"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) RELEASE=1; shift ;;
    --out) OUT_ROOT="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

PROFILE="debug"
CARGO_ARGS=(build -p legion-desktop)
if [[ "$RELEASE" -eq 1 ]]; then
  PROFILE="release"
  CARGO_ARGS+=(--release)
fi

OS_NAME="$(uname -s | tr '[:upper:]' '[:lower:]')"
case "$OS_NAME" in
  darwin) OS_LABEL="macos" ;;
  linux) OS_LABEL="linux" ;;
  *) OS_LABEL="$OS_NAME" ;;
esac

ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
  x86_64|amd64) ARCH="x64" ;;
  aarch64|arm64) ARCH="arm64" ;;
  *) ARCH="$ARCH_RAW" ;;
esac

BUNDLE_DIR="${OUT_ROOT}/${OS_LABEL}-${ARCH}"
SOURCE_BIN="target/${PROFILE}/legion-desktop"
DEST_BIN="${BUNDLE_DIR}/legion-desktop"
ARCHIVE="${OUT_ROOT}/legion-desktop-preview-${OS_LABEL}-${ARCH}.tar.gz"

echo "Legion preview package (unsigned-beta)"
echo "Repository: $REPO_ROOT"
echo "Profile: $PROFILE"
echo "Bundle: $BUNDLE_DIR"

cargo "${CARGO_ARGS[@]}"

if [[ ! -f "$SOURCE_BIN" ]]; then
  echo "expected binary missing: $SOURCE_BIN" >&2
  exit 1
fi

mkdir -p "$BUNDLE_DIR"
cp -f "$SOURCE_BIN" "$DEST_BIN"
chmod +x "$DEST_BIN"

GIT_SHA="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
BUILT_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

cat > "${BUNDLE_DIR}/UNSIGNED-BETA.toml" <<EOF
schema_version = 1
package = "legion-desktop"
channel = "preview"
profile = "${PROFILE}"
platform = "${OS_LABEL}"
arch = "${ARCH}"
git_sha = "${GIT_SHA}"
built_at_utc = "${BUILT_AT}"
signer_status = "unsigned-beta/no-os-code-signing"
os_code_signing = false
production = false
notes = "Portable unsigned preview. Not OS code-signed / notarized. Do not distribute as a production release."
EOF

cat > "${BUNDLE_DIR}/package-manifest.txt" <<EOF
package: legion-desktop
channel: preview
platform: ${OS_LABEL}
arch: ${ARCH}
profile: ${PROFILE}
git_sha: ${GIT_SHA}
built_at_utc: ${BUILT_AT}
signer_status: unsigned-beta/no-os-code-signing
package_executable: ${DEST_BIN}
EOF

mkdir -p "$OUT_ROOT"
tar -C "$BUNDLE_DIR" -czf "$ARCHIVE" .

echo "Wrote ${BUNDLE_DIR}/UNSIGNED-BETA.toml"
echo "Wrote ${BUNDLE_DIR}/package-manifest.txt"
echo "Wrote $ARCHIVE"
