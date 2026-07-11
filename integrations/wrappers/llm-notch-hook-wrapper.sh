#!/usr/bin/env sh
# llm_notch hook wrapper — POSIX shell, no jq/python/node required.
# Always fails open in hook mode (exit 0) so vendor agents keep working.
#
# Usage:
#   llm-notch-hook-wrapper.sh --source <cursor|claudeCode|codex|generic> --vendor-event <name>
#   (vendor JSON on stdin; forwarded to llm-notch-hook when available)
#
# Environment:
#   LLM_NOTCH_HOOK_BIN   — path to signed helper (default: llm-notch-hook)
#   LLM_NOTCH_HOOK_TIMEOUT_SEC — max wait seconds (default: 2)

set -eu

SOURCE=""
VENDOR_EVENT=""
TIMEOUT_SEC="${LLM_NOTCH_HOOK_TIMEOUT_SEC:-2}"
HELPER="${LLM_NOTCH_HOOK_BIN:-llm-notch-hook}"

fail_open() {
  # Observation-only integrations return empty JSON on stdout for Cursor-compatible hooks.
  printf '%s\n' '{}'
  exit 0
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --source)
      SOURCE="${2:-}"
      shift 2
      ;;
    --vendor-event)
      VENDOR_EVENT="${2:-}"
      shift 2
      ;;
    --timeout-sec)
      TIMEOUT_SEC="${2:-2}"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

if [ -z "$SOURCE" ] || [ -z "$VENDOR_EVENT" ]; then
  fail_open
fi

if ! command -v "$HELPER" >/dev/null 2>&1; then
  fail_open
fi

# Buffer stdin once so we can enforce a timeout without losing payload.
TMP_IN=""
cleanup() {
  if [ -n "$TMP_IN" ] && [ -f "$TMP_IN" ]; then
    rm -f "$TMP_IN"
  fi
}
trap cleanup EXIT INT TERM

TMP_IN="$(mktemp "${TMPDIR:-/tmp}/llm-notch-hook.XXXXXX")" || fail_open
cat >"$TMP_IN" || fail_open

# Bounded wait without requiring GNU timeout (not present on stock macOS).
(
  "$HELPER" hook \
    --source "$SOURCE" \
    --vendor-event "$VENDOR_EVENT" \
    --hook-mode \
    <"$TMP_IN" \
    >/dev/null 2>&1 &
  hp=$!
  (
    sleep "$TIMEOUT_SEC" 2>/dev/null || sleep 2
    kill "$hp" 2>/dev/null || true
  ) &
  kp=$!
  wait "$hp" 2>/dev/null || true
  kill "$kp" 2>/dev/null || true
  wait "$kp" 2>/dev/null || true
) || true

fail_open
