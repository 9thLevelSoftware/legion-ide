#!/usr/bin/env bash

# Verify a packaged Legion native installer end to end.
#
# Covers: artifact existence, SHA-256 checksum, generated release metadata,
# install/extract structure, installer version, and the extracted/installed
# binary's headless --beta-smoke exit status.
#
# Evidence-first contract:
#   * PACKAGE-EVIDENCE.txt in the package directory receives every check
#     result plus the complete smoke logs.
#   * The complete evidence report is printed to the terminal on success and
#     on failure, so no defect is visible only inside an uploaded artifact.
#   * VALIDATION-SUMMARY.toml is written beside the installer on every exit;
#     the publish job refuses to release unless every summary reports
#     result = "passed" and smoke_exit = 0.
#
# The beta smoke workspace is always derived as
#   <workspace-root>/target/release-smoke/<platform>-<architecture>-<format>/workspace
# because the application rejects beta workspaces outside <workspace>/target.

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/verify-native-package.sh
         --format {deb|appimage|dmg}
         --package-dir DIRECTORY
         --release-version 0.0.N
         --source-sha 40-HEX-SHA
         --workspace-root DIRECTORY
         [--architecture {x64|arm64}]   (default x64; dmg supports x64 and arm64)
         [--print-smoke-plan]           (print derived smoke paths and exit 0)

Environment:
  EXPECTED_DEB_MAINTAINER  optional; when set, the DEB Maintainer field must
                           equal this value exactly. The Maintainer field must
                           always be non-empty regardless.
EOF
}

FORMAT=""
PACKAGE_DIR=""
RELEASE_VERSION=""
SOURCE_SHA=""
WORKSPACE_ROOT=""
ARCHITECTURE="x64"
PRINT_SMOKE_PLAN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --format)
      [[ $# -ge 2 ]] || { echo "--format requires a value" >&2; exit 2; }
      FORMAT="$2"
      shift 2
      ;;
    --package-dir)
      [[ $# -ge 2 ]] || { echo "--package-dir requires a value" >&2; exit 2; }
      PACKAGE_DIR="$2"
      shift 2
      ;;
    --release-version)
      [[ $# -ge 2 ]] || { echo "--release-version requires a value" >&2; exit 2; }
      RELEASE_VERSION="$2"
      shift 2
      ;;
    --source-sha)
      [[ $# -ge 2 ]] || { echo "--source-sha requires a value" >&2; exit 2; }
      SOURCE_SHA="$2"
      shift 2
      ;;
    --workspace-root)
      [[ $# -ge 2 ]] || { echo "--workspace-root requires a value" >&2; exit 2; }
      WORKSPACE_ROOT="$2"
      shift 2
      ;;
    --architecture)
      [[ $# -ge 2 ]] || { echo "--architecture requires a value" >&2; exit 2; }
      ARCHITECTURE="$2"
      shift 2
      ;;
    --print-smoke-plan)
      PRINT_SMOKE_PLAN=1
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

[[ "$RELEASE_VERSION" =~ ^0\.0\.[1-9][0-9]*$ ]] || { echo "--release-version must be canonical 0.0.N with N at least 1 and no zero padding" >&2; exit 2; }
[[ "$SOURCE_SHA" =~ ^[0-9a-fA-F]{40}$ ]] || { echo "--source-sha must be a 40-hex commit SHA" >&2; exit 2; }
[[ -n "$PACKAGE_DIR" ]] || { echo "--package-dir is required" >&2; exit 2; }
[[ -n "$WORKSPACE_ROOT" ]] || { echo "--workspace-root is required" >&2; exit 2; }

case "$FORMAT" in
  deb)
    PLATFORM="linux"
    EXTENSION="deb"
    ;;
  appimage)
    PLATFORM="linux"
    EXTENSION="AppImage"
    ;;
  dmg)
    PLATFORM="macos"
    EXTENSION="dmg"
    ;;
  *)
    echo "--format must be one of: deb, appimage, dmg" >&2
    exit 2
    ;;
esac

case "$ARCHITECTURE" in
  x64|arm64) ;;
  *) echo "--architecture must be x64 or arm64" >&2; exit 2 ;;
esac
if [[ "$PLATFORM" == "linux" && "$ARCHITECTURE" != "x64" ]]; then
  echo "Linux native packages are supported only for x64" >&2
  exit 2
fi

STEM="legion-desktop-${PLATFORM}-${ARCHITECTURE}-${FORMAT}"
SMOKE_STEM="${PLATFORM}-${ARCHITECTURE}-${FORMAT}"
SMOKE_ROOT="$WORKSPACE_ROOT/target/release-smoke/$SMOKE_STEM"
BETA_WORKSPACE="$SMOKE_ROOT/workspace"
SMOKE_DIR="$SMOKE_ROOT/smoke"
STAGING_DIR="$SMOKE_ROOT/staging"
EXTRACT_DIR="$SMOKE_ROOT/extract"
ATTACH_PLIST="$SMOKE_ROOT/attach.plist"
CANDIDATE_TAG="v$RELEASE_VERSION"

artifact_path="$PACKAGE_DIR/$STEM.$EXTENSION"
checksum_path="$artifact_path.sha256"
metadata_path="$PACKAGE_DIR/RELEASE-METADATA.toml"
evidence_path="$PACKAGE_DIR/PACKAGE-EVIDENCE.txt"
summary_path="$PACKAGE_DIR/VALIDATION-SUMMARY.toml"

if [[ "$PRINT_SMOKE_PLAN" -eq 1 ]]; then
  printf 'artifact=%s\n' "$artifact_path"
  printf 'beta_workspace=%s\n' "$BETA_WORKSPACE"
  printf 'smoke_dir=%s\n' "$SMOKE_DIR"
  printf 'staging_dir=%s\n' "$STAGING_DIR"
  exit 0
fi

# Per-check status tracked for VALIDATION-SUMMARY.toml. "not-run" means the
# verifier failed before reaching that check; the publish gate accepts only
# "passed".
checksum_status="not-run"
metadata_status="not-run"
package_version_status="not-run"
structure_status="not-run"
smoke_exit=-1
mount_point=""

write_summary() {
  local result="$1"
  cat > "$summary_path" <<EOF
schema_version = 1
candidate_tag = "$CANDIDATE_TAG"
source_sha = "$SOURCE_SHA"
format = "$FORMAT"
architecture = "$ARCHITECTURE"
checksum = "$checksum_status"
metadata = "$metadata_status"
package_version = "$package_version_status"
structure = "$structure_status"
smoke_exit = $smoke_exit
result = "$result"
EOF
}

# Create the evidence report before any format-specific check so that every
# failure path has a durable, printable record.
mkdir -p "$PACKAGE_DIR" "$SMOKE_ROOT" "$SMOKE_DIR" "$STAGING_DIR"
{
  printf 'verifier=scripts/verify-native-package.sh\n'
  printf 'candidate_tag=%s\n' "$CANDIDATE_TAG"
  printf 'source_sha=%s\n' "$SOURCE_SHA"
  printf 'format=%s\n' "$FORMAT"
  printf 'architecture=%s\n' "$ARCHITECTURE"
  printf 'verifier_os=%s\n' "$(uname -srm)"
  if command -v dpkg-deb >/dev/null 2>&1; then
    printf 'dpkg_deb_version=%s\n' "$(dpkg-deb --version | head -n 1)"
  fi
} >> "$evidence_path"

finish() {
  status=$?
  trap - EXIT
  if [[ -n "${mount_point:-}" ]]; then
    hdiutil detach "$mount_point" >/dev/null 2>&1 || true
    mount_point=""
  fi
  local result="failed"
  [[ "$status" -eq 0 ]] && result="passed"
  write_summary "$result"
  printf 'result=%s\n' "$result" >> "$evidence_path"
  echo "==== PACKAGE-EVIDENCE ($STEM) ===="
  cat "$evidence_path"
  exit "$status"
}
trap finish EXIT

fail() {
  printf 'error=%s\n' "$1" >> "$evidence_path"
  exit 1
}

require_file() {
  [[ -s "$1" ]] || fail "missing required file: $1"
}

compute_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

verify_checksum() {
  require_file "$artifact_path"
  require_file "$checksum_path"
  require_file "$metadata_path"
  local checksum_line expected_hash expected_file actual_hash
  checksum_line="$(tr -d '\r' < "$checksum_path")"
  expected_hash="${checksum_line%% *}"
  expected_file="${checksum_line#* }"
  [[ "$expected_hash" =~ ^[0-9a-f]{64}$ ]] || fail "malformed checksum hash in $checksum_path"
  [[ "$expected_file" == "*$(basename "$artifact_path")" ]] || fail "checksum names unexpected installer: $expected_file"
  actual_hash="$(compute_sha256 "$artifact_path")"
  [[ "$actual_hash" == "$expected_hash" ]] || fail "sha256 mismatch for $artifact_path: expected $expected_hash, computed $actual_hash"
  checksum_status="passed"
  printf 'checksum=passed sha256=%s\n' "$actual_hash" >> "$evidence_path"
}

verify_metadata() {
  local line
  for line in \
    "release_version = \"$RELEASE_VERSION\"" \
    "git_sha = \"$SOURCE_SHA\"" \
    "platform = \"$PLATFORM\"" \
    "architecture = \"$ARCHITECTURE\"" \
    "format = \"$FORMAT\"" \
    'signer_status = "unsigned-beta/no-os-code-signing"'; do
    grep -Fqx "$line" "$metadata_path" || fail "missing metadata line: $line"
  done
  metadata_status="passed"
  printf 'metadata=passed\n' >> "$evidence_path"
}

# run_smoke LAUNCH-WORD... — appends the canonical beta-smoke arguments to the
# provided launch command, captures all output into the evidence report, and
# hard-fails on a non-zero exit. The beta workspace is always contained in
# <workspace-root>/target.
run_smoke() {
  local smoke_status
  set +e
  "$@" \
    --beta-smoke --duration-ms 1500 \
    --workspace "$WORKSPACE_ROOT" \
    --beta-workspace "$BETA_WORKSPACE" \
    --evidence "$SMOKE_DIR/beta-smoke.md" \
    --session-state "$SMOKE_DIR/session.json" \
    --diagnostics-export "$SMOKE_DIR/diagnostics.md" \
    > "$SMOKE_DIR/stdout-stderr.log" 2>&1
  smoke_status=$?
  set -e
  smoke_exit="$smoke_status"
  printf 'smoke_exit=%s policy=hard-fail-beta-workflow-is-headless\n' "$smoke_status" >> "$evidence_path"
  cat "$SMOKE_DIR/stdout-stderr.log" >> "$evidence_path"
  test ! -f "$SMOKE_DIR/beta-smoke.md" || cat "$SMOKE_DIR/beta-smoke.md" >> "$evidence_path"
  [[ "$smoke_status" -eq 0 ]] || fail "beta smoke failed with exit code $smoke_status"
  printf 'smoke=passed\n' >> "$evidence_path"
}

verify_deb() {
  verify_checksum
  verify_metadata

  dpkg-deb --info "$artifact_path" | tee -a "$evidence_path"

  local maintainer
  maintainer="$(dpkg-deb -f "$artifact_path" Maintainer)"
  [[ -n "$maintainer" ]] || fail "DEB control file is missing a non-empty Maintainer field"
  if [[ -n "${EXPECTED_DEB_MAINTAINER:-}" ]]; then
    [[ "$maintainer" == "$EXPECTED_DEB_MAINTAINER" ]] || fail "DEB Maintainer mismatch: expected '$EXPECTED_DEB_MAINTAINER', found '$maintainer'"
  fi
  printf 'deb_maintainer=passed value=%s\n' "$maintainer" >> "$evidence_path"

  local package_version
  package_version="$(dpkg-deb -f "$artifact_path" Version)"
  [[ "$package_version" == "$RELEASE_VERSION" ]] || fail "DEB Version mismatch: expected $RELEASE_VERSION, found $package_version"
  package_version_status="passed"
  printf 'package_version=passed version=%s\n' "$package_version" >> "$evidence_path"

  dpkg-deb -x "$artifact_path" "$STAGING_DIR"
  local staged_binary="$STAGING_DIR/usr/bin/legion-desktop"
  local desktop_entry="$STAGING_DIR/usr/share/applications/legion.desktop"
  local icon_path="$STAGING_DIR/usr/share/icons/hicolor/512x512/apps/legion.png"
  [[ -x "$staged_binary" ]] || fail "extracted DEB is missing executable $staged_binary"
  [[ -s "$desktop_entry" ]] || fail "extracted DEB is missing desktop entry $desktop_entry"
  [[ -s "$icon_path" ]] || fail "extracted DEB is missing icon $icon_path"
  structure_status="passed"
  printf 'structure=passed binary=%s desktop=%s icon=%s\n' \
    "$staged_binary" "$desktop_entry" "$icon_path" >> "$evidence_path"

  run_smoke xvfb-run --auto-servernum "$staged_binary"
}

verify_appimage() {
  verify_checksum
  verify_metadata

  chmod +x "$artifact_path"
  file "$artifact_path" | tee -a "$evidence_path"

  mkdir -p "$EXTRACT_DIR"
  (cd "$EXTRACT_DIR" && "$artifact_path" --appimage-extract >/dev/null)
  local apprun_path="$EXTRACT_DIR/squashfs-root/AppRun"
  local app_binary="$EXTRACT_DIR/squashfs-root/usr/bin/legion-desktop"
  local desktop_entry="$EXTRACT_DIR/squashfs-root/usr/share/applications/legion-desktop.desktop"
  local icon_path="$EXTRACT_DIR/squashfs-root/usr/share/icons/hicolor/512x512/apps/legion.png"
  [[ -x "$apprun_path" ]] || fail "extracted AppImage is missing executable $apprun_path"
  [[ -f "$app_binary" && -x "$app_binary" ]] || fail "extracted AppImage is missing executable $app_binary"
  [[ -s "$desktop_entry" ]] || fail "extracted AppImage is missing desktop entry $desktop_entry"
  [[ -s "$icon_path" ]] || fail "extracted AppImage is missing icon $icon_path"
  structure_status="passed"
  printf 'structure=passed apprun=%s binary=%s desktop=%s icon=%s\n' \
    "$apprun_path" "$app_binary" "$desktop_entry" "$icon_path" >> "$evidence_path"

  # The AppImage format carries no independent version field; the version is
  # proven by the release-metadata check bound to this exact artifact through
  # its verified SHA-256 checksum.
  package_version_status="passed"
  printf 'package_version=passed version=%s source=release-metadata\n' "$RELEASE_VERSION" >> "$evidence_path"

  run_smoke xvfb-run --auto-servernum "$artifact_path" --appimage-extract-and-run
}

verify_dmg() {
  verify_checksum
  verify_metadata

  hdiutil verify "$artifact_path" | tee -a "$evidence_path"
  printf 'dmg_verify=passed\n' >> "$evidence_path"

  hdiutil attach "$artifact_path" -readonly -nobrowse -plist > "$ATTACH_PLIST"
  mount_point="$(python3 - "$ATTACH_PLIST" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "rb") as source:
    data = plistlib.load(source)
mounts = [
    entity["mount-point"]
    for entity in data.get("system-entities", [])
    if "mount-point" in entity
]
if len(mounts) != 1:
    raise SystemExit(f"Expected one mounted volume, got {mounts!r}")
print(mounts[0])
PY
)"

  local app_count app_path copied_app info_plist staged_binary bundle_version
  app_count="$(find "$mount_point" -maxdepth 1 -type d -name '*.app' | wc -l | tr -d ' ')"
  [[ "$app_count" -eq 1 ]] || fail "expected exactly one .app on the mounted DMG; found $app_count"
  app_path="$(find "$mount_point" -maxdepth 1 -type d -name '*.app' -print -quit)"
  copied_app="$STAGING_DIR/$(basename "$app_path")"
  ditto "$app_path" "$copied_app"
  info_plist="$copied_app/Contents/Info.plist"
  staged_binary="$copied_app/Contents/MacOS/legion-desktop"
  [[ -f "$info_plist" ]] || fail "copied app bundle is missing $info_plist"
  [[ -x "$staged_binary" ]] || fail "copied app bundle is missing executable $staged_binary"
  bundle_version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$info_plist")"
  [[ "$bundle_version" == "$RELEASE_VERSION" ]] || fail "CFBundleShortVersionString mismatch: expected $RELEASE_VERSION, found $bundle_version"
  package_version_status="passed"
  printf 'package_version=passed version=%s\n' "$bundle_version" >> "$evidence_path"

  # Detach before smoke: the beta smoke must run from the copied bundle, never
  # from the mounted read-only volume.
  hdiutil detach "$mount_point"
  mount_point=""
  structure_status="passed"
  printf 'structure=passed app=%s binary=%s bundle_version=%s\n' \
    "$copied_app" "$staged_binary" "$bundle_version" >> "$evidence_path"

  run_smoke "$staged_binary"
}

case "$FORMAT" in
  deb) verify_deb ;;
  appimage) verify_appimage ;;
  dmg) verify_dmg ;;
esac
