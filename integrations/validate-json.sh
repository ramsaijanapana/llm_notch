#!/usr/bin/env sh
# Validate JSON syntax for all fixtures under integrations/fixtures.
# Uses the first available validator on the system; adds no repo dependencies.
set -eu

ROOT="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
FIXTURES="$ROOT/fixtures"
FAILED=0
CHECKED=0
VALIDATOR=""

validate_file() {
  file="$1"
  CHECKED=$((CHECKED + 1))
  case "$VALIDATOR" in
    python3)
      python3 -m json.tool "$file" >/dev/null 2>&1 || { echo "INVALID: $file" >&2; FAILED=1; }
      ;;
    node)
      node -e "JSON.parse(require('fs').readFileSync(process.argv[1],'utf8'))" "$file" >/dev/null 2>&1 || { echo "INVALID: $file" >&2; FAILED=1; }
      ;;
    ruby)
      ruby -rjson -e "JSON.parse(File.read(ARGV[0]))" "$file" >/dev/null 2>&1 || { echo "INVALID: $file" >&2; FAILED=1; }
      ;;
    jq)
      jq -e . "$file" >/dev/null 2>&1 || { echo "INVALID: $file" >&2; FAILED=1; }
      ;;
    *)
      echo "No JSON validator found (tried python3, node, ruby, jq)." >&2
      exit 2
      ;;
  esac
}

if command -v python3 >/dev/null 2>&1; then
  VALIDATOR=python3
elif command -v node >/dev/null 2>&1; then
  VALIDATOR=node
elif command -v ruby >/dev/null 2>&1; then
  VALIDATOR=ruby
elif command -v jq >/dev/null 2>&1; then
  VALIDATOR=jq
fi

# Also validate template JSON (strip leading _comment keys by copying through validator round-trip when possible)
for file in $(find "$FIXTURES" -name '*.json' | sort); do
  validate_file "$file"
done

for file in \
  "$ROOT/cursor/hooks.json.template" \
  "$ROOT/claude-code/settings.hooks.template.json" \
  "$ROOT/codex/hooks.json.template" \
  "$ROOT/gemini/settings.hooks.template.json" \
  "$ROOT/qwen/settings.hooks.template.json" \
  "$ROOT/antigravity-cli/hooks.json.template" \
  "$ROOT/copilot/hooks.json.template" \
  "$ROOT/remote/hooks.cursor.template.json"
do
  if [ -f "$file" ]; then
    validate_file "$file"
  fi
done

echo "Validated $CHECKED JSON files using $VALIDATOR."
if [ "$FAILED" -ne 0 ]; then
  exit 1
fi
