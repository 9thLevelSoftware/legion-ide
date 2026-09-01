#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/package-native.sh --version 0.0.N --format {dmg|deb|appimage} [--out-dir DIRECTORY] [--dry-run]
       N must be at least 1 with no zero padding (for example, 0.0.1).
EOF
}

VERSION=""
FORMAT=""
OUT_DIR="target/native-package/output"
DRY_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      [[ $# -ge 2 ]] || { echo "--version requires a value" >&2; exit 2; }
      VERSION="$2"
      shift 2
      ;;
    --format)
      [[ $# -ge 2 ]] || { echo "--format requires a value" >&2; exit 2; }
      FORMAT="$2"
      shift 2
      ;;
    --out-dir|--output-dir|--out)
      [[ $# -ge 2 ]] || { echo "$1 requires a value" >&2; exit 2; }
      OUT_DIR="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ "$VERSION" =~ ^0\.0\.[1-9][0-9]*$ ]] || { echo "--version must be canonical 0.0.N with N at least 1 and no zero padding" >&2; exit 2; }
case "$FORMAT" in
  dmg|deb|appimage) ;;
  *) echo "--format must be one of: dmg, deb, appimage" >&2; exit 2 ;;
esac

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
NATIVE_DIR="$REPO_ROOT/target/native-package"
OUTPUT_DIR="$OUT_DIR"
if [[ "$OUTPUT_DIR" != /* ]]; then
  OUTPUT_DIR="$REPO_ROOT/$OUTPUT_DIR"
fi

case "$FORMAT" in
  dmg)
    PLATFORM="macos"
    EXTENSION="dmg"
    ;;
  deb)
    PLATFORM="linux"
    EXTENSION="deb"
    ;;
  appimage)
    PLATFORM="linux"
    EXTENSION="AppImage"
    ;;
esac

case "$(uname -m)" in
  x86_64|amd64) ARCH="x64" ;;
  aarch64|arm64) ARCH="arm64" ;;
  *) echo "unsupported architecture: $(uname -m)" >&2; exit 2 ;;
esac
if [[ "$PLATFORM" == "linux" && "$ARCH" != "x64" ]]; then
  echo "Linux native packages are supported only for x64" >&2
  exit 2
fi

PACKAGE_NAME="legion-desktop-${PLATFORM}-${ARCH}-${FORMAT}.${EXTENSION}"
PACKAGE_PATH="$OUTPUT_DIR/$PACKAGE_NAME"
CONFIG_PATH="$NATIVE_DIR/Packager.toml"
BINARIES_DIR="$NATIVE_DIR/cargo-target/release"
PACKAGING_DIR="$REPO_ROOT/packaging"
STAGING_DIR="$NATIVE_DIR/packager-${FORMAT}-$$"

toml_escape() {
  printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'
}

sed_escape() {
  printf '%s' "$1" | sed -e 's/[\\&|]/\\&/g'
}

render_config() {
  mkdir -p "$NATIVE_DIR"
  sed \
    -e "s|^version = \"0\\.0\\.0\"$|version = \"$VERSION\"|" \
    -e "s|__FORMAT__|$(sed_escape "$FORMAT")|g" \
    -e "s|__BINARIES_DIR__|$(sed_escape "$(toml_escape "$BINARIES_DIR")")|g" \
    -e "s|__OUT_DIR__|$(sed_escape "$(toml_escape "$STAGING_DIR")")|g" \
    -e "s|__PACKAGING_DIR__|$(sed_escape "$(toml_escape "$PACKAGING_DIR")")|g" \
    -e "s|__REPO_ROOT__|$(sed_escape "$(toml_escape "$REPO_ROOT")")|g" \
    "$PACKAGING_DIR/Packager.toml" > "$CONFIG_PATH"
}

render_config
if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "Planned package: $PACKAGE_PATH"
  exit 0
fi

if [[ -e "$PACKAGE_PATH" ]]; then
  echo "refusing to overwrite existing package: $PACKAGE_PATH" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"
mkdir "$STAGING_DIR"
trap 'rm -rf -- "$STAGING_DIR"' EXIT
CARGO_TARGET_DIR="$NATIVE_DIR/cargo-target" cargo build --release -p legion-desktop
mkdir -p "$BINARIES_DIR"
cp "$REPO_ROOT/LICENSE" "$BINARIES_DIR/LICENSE"
cp "$REPO_ROOT/docs/PRIVACY.md" "$BINARIES_DIR/PRIVACY.md"
cp "$REPO_ROOT/THIRD_PARTY_NOTICES.md" "$BINARIES_DIR/THIRD_PARTY_NOTICES.md"
CARGO_TARGET_DIR="$NATIVE_DIR/cargo-target" cargo packager --release --config "$CONFIG_PATH"

PACKAGE_CANDIDATE=""
PACKAGE_CANDIDATE_COUNT=0
while IFS= read -r -d '' candidate; do
  PACKAGE_CANDIDATE_COUNT=$((PACKAGE_CANDIDATE_COUNT + 1))
  if [[ "$PACKAGE_CANDIDATE_COUNT" -eq 1 ]]; then
    PACKAGE_CANDIDATE="$candidate"
  fi
done < <(find "$STAGING_DIR" -type f -name "*.$EXTENSION" -print0)
if [[ "$PACKAGE_CANDIDATE_COUNT" -ne 1 ]]; then
  echo "expected exactly one .$EXTENSION package in $STAGING_DIR; found $PACKAGE_CANDIDATE_COUNT" >&2
  exit 1
fi
if [[ "$FORMAT" == "appimage" && "$(basename "$PACKAGE_CANDIDATE")" != *"$VERSION"* ]]; then
  echo "generated AppImage candidate filename must contain requested version $VERSION: $(basename "$PACKAGE_CANDIDATE")" >&2
  exit 1
fi

mv -- "$PACKAGE_CANDIDATE" "$PACKAGE_PATH"

WORKSPACE_VERSION="$(sed -n '/^\[workspace\.package\]/,/^\[/ { s/^version = "\([^"]*\)"/\1/p; }' Cargo.toml)"
GIT_SHA="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
cat > "$OUTPUT_DIR/RELEASE-METADATA.toml" <<EOF
release_version = "$VERSION"
workspace_version = "$WORKSPACE_VERSION"
git_sha = "$GIT_SHA"
platform = "$PLATFORM"
architecture = "$ARCH"
format = "$FORMAT"
signer_status = "unsigned-beta/no-os-code-signing"
EOF

if command -v sha256sum >/dev/null 2>&1; then
  PACKAGE_SHA256="$(sha256sum -- "$PACKAGE_PATH" | awk '{print $1}')"
else
  PACKAGE_SHA256="$(shasum -a 256 "$PACKAGE_PATH" | awk '{print $1}')"
fi
printf '%s *%s\n' "$PACKAGE_SHA256" "$PACKAGE_NAME" > "$PACKAGE_PATH.sha256"

echo "Wrote $PACKAGE_PATH"
echo "Wrote $OUTPUT_DIR/RELEASE-METADATA.toml"
echo "Wrote $PACKAGE_PATH.sha256"
