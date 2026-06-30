#!/usr/bin/env bash
# Check all Markdown files under docs/ using `cr docs check-md`.
# Usage: ./scripts/check-docs-md.sh [entry-file]
#   entry-file defaults to calcit/test.cirru in this repository.

set -eo pipefail

ENTRY="${1:-calcit/test.cirru}"

# Prefer a pre-built debug binary; fall back to cargo run (slower but always works).
if [ -x "./target/debug/cr" ]; then
  CR_CMD=("./target/debug/cr")
elif [ -x "./target/release/cr" ]; then
  CR_CMD=("./target/release/cr")
else
  CR_CMD=(cargo run --bin cr --)
fi

FAILED=0
TOTAL=0
FAILED_BLOCKS=0
TOTAL_BLOCKS=0
QUIET_ARGS=()

if [[ "${CI:-}" == "true" ]]; then
  QUIET_ARGS+=(--quiet)
fi

while IFS= read -r file; do
  TOTAL=$((TOTAL + 1))

  output=""
  exit_code=0
  if output="$("${CR_CMD[@]}" "$ENTRY" docs check-md "$file" --entry "$ENTRY" "${QUIET_ARGS[@]}" 2>&1)"; then
    :
  else
    exit_code=$?
  fi

  if [ -n "$output" ]; then
    printf '%s\n' "$output"
  fi

  summary_line=""
  while IFS= read -r line; do
    if [[ "$line" == Results:* ]]; then
      summary_line="$line"
    fi
  done <<< "$output"

  if [[ "$summary_line" =~ Results:\ ([0-9]+)\ blocks,\ ([0-9]+)\ passed,\ ([0-9]+)\ failed ]]; then
    TOTAL_BLOCKS=$((TOTAL_BLOCKS + BASH_REMATCH[1]))
    FAILED_BLOCKS=$((FAILED_BLOCKS + BASH_REMATCH[3]))
  fi

  if [ "$exit_code" -ne 0 ]; then
    FAILED=$((FAILED + 1))
  fi
done < <(find docs -name '*.md' | sort)

echo ""
echo "Docs check-md: ${TOTAL} files, $((TOTAL - FAILED)) passed, ${FAILED} failed; ${TOTAL_BLOCKS} blocks, $((TOTAL_BLOCKS - FAILED_BLOCKS)) passed, ${FAILED_BLOCKS} failed"

if [ "${FAILED}" -gt 0 ] || [ "${FAILED_BLOCKS}" -gt 0 ]; then
  exit 1
fi
