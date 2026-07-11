#!/usr/bin/env sh
# Safe generic emit examples — require explicit llm-notch-hook on PATH and a running llm_notch host.
# These are documentation samples; they are not invoked by any installer.
set -eu

HELPER="${LLM_NOTCH_HOOK_BIN:-llm-notch-hook}"

if ! command -v "$HELPER" >/dev/null 2>&1; then
  echo "llm-notch-hook not found; examples are inert." >&2
  exit 1
fi

# Start a generic session without claiming process attribution.
# To enable attribution, add BOTH --pid and --process-started-at-ms using
# values obtained from the same live process identity.
"$HELPER" emit \
  --source generic \
  --event sessionStart \
  --external-session-id generic-cli-7 \
  --label "Generic CLI agent" \
  --workspace-label llm_notch \
  --status running

# Append a redacted tool event.
"$HELPER" emit \
  --source generic \
  --event tool \
  --external-session-id generic-cli-7 \
  --summary "Build step finished" \
  --tool-name cargo

# Set observation-only attention.
"$HELPER" emit \
  --source generic \
  --event attention \
  --external-session-id generic-cli-7 \
  --attention question \
  --summary "Agent waiting for input"
