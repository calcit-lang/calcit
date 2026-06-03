#!/usr/bin/env bash
# Check all Markdown files under docs/ using `cr docs check-md`.
# Usage: ./scripts/check-docs-md.sh [entry-file]
#   entry-file defaults to demos/calcit.cirru

set -eo pipefail

ENTRY="${1:-calcit/test.cirru}"

# Prefer a pre-built debug binary; fall back to cargo run (slower but always works).
if [ -x "./target/debug/cr" ]; then
  CR_CMD="./target/debug/cr"
elif [ -x "./target/release/cr" ]; then
  CR_CMD="./target/release/cr"
else
  CR_CMD="cargo run --bin cr --"
fi

FAILED=0
TOTAL=0
QUIET_ARGS=()

if [[ "${CI:-}" == "true" ]]; then
  QUIET_ARGS+=(--quiet)
fi

while IFS= read -r file; do
  TOTAL=$((TOTAL + 1))
  if $CR_CMD "$ENTRY" docs check-md "$file" -d "$ENTRY" "${QUIET_ARGS[@]}" 2>&1; then
    :
  else
    FAILED=$((FAILED + 1))
  fi
done < <(find docs -name '*.md' | sort)

echo ""
echo "Docs check-md: ${TOTAL} files, $((TOTAL - FAILED)) passed, ${FAILED} failed"

if [ "${FAILED}" -gt 0 ]; then
  exit 1
fi
