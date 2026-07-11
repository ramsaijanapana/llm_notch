#!/usr/bin/env bash
# macOS notarization scaffold for llm_notch release builds.
#
# Required env (never commit):
#   APPLE_ID, APPLE_ID_PASSWORD, APPLE_TEAM_ID
#   CODESIGN_IDENTITY — e.g. "Developer ID Application: Your Name (TEAMID)"
#
# Usage:
#   ./scripts/signing/notarize-macos.sh path/to/llm_notch.app path/to/llm_notch.dmg

set -euo pipefail

APP_PATH="${1:-}"
DMG_PATH="${2:-}"
IDENTITY="${CODESIGN_IDENTITY:-}"

if [[ -z "$APP_PATH" || -z "$DMG_PATH" ]]; then
  echo "usage: $0 <llm_notch.app> <llm_notch.dmg>" >&2
  exit 1
fi

if [[ -z "${APPLE_ID:-}" || -z "${APPLE_ID_PASSWORD:-}" || -z "${APPLE_TEAM_ID:-}" ]]; then
  echo "error: APPLE_ID, APPLE_ID_PASSWORD, and APPLE_TEAM_ID are required for notarization." >&2
  echo "CI builds remain unsigned; this script is a release gate scaffold only." >&2
  exit 2
fi

if [[ -z "$IDENTITY" ]]; then
  echo "error: set CODESIGN_IDENTITY to a Developer ID Application certificate." >&2
  exit 2
fi

HELPER="$APP_PATH/Contents/MacOS/llm-notch-hook"
if [[ ! -f "$HELPER" ]]; then
  echo "error: bundled helper missing at $HELPER (check externalBin / native:prepare-helper)" >&2
  exit 1
fi

echo "Signing helper..."
codesign --force --options runtime --timestamp --sign "$IDENTITY" "$HELPER"

echo "Signing app..."
codesign --force --deep --options runtime --timestamp --sign "$IDENTITY" "$APP_PATH"
codesign --verify --deep --strict "$APP_PATH"

echo "Signing DMG..."
codesign --force --timestamp --sign "$IDENTITY" "$DMG_PATH"

echo "Submitting for notarization..."
xcrun notarytool submit "$DMG_PATH" \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_ID_PASSWORD" \
  --team-id "$APPLE_TEAM_ID" \
  --wait

echo "Stapling..."
xcrun stapler staple "$DMG_PATH"
spctl --assess --type open --context context:primary-signature -v "$DMG_PATH"

echo "Notarization gate passed for $DMG_PATH"
