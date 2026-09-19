#!/usr/bin/env bash
# Check all Markdown files under docs/ using `calcit docs check-md`.
# Usage: ./scripts/check-docs-md.sh [entry-file]
#   entry-file defaults to calcit/test.cirru in this repository.
# Set CALCIT_DOCS_CHECK_JOBS to override the parallelism (default: 4).
#
# Each `docs check-md` invocation has to load the shared core snapshot, so the
# files are checked concurrently with a bounded number of processes instead of
# starting one process per file strictly sequentially.

set -eo pipefail

ENTRY="${1:-calcit/test.cirru}"

# Prefer a pre-built debug binary; fall back to cargo run (slower but always works).
if [ -x "./target/debug/calcit" ]; then
  CR_CMD=("./target/debug/calcit")
elif [ -x "./target/release/calcit" ]; then
  CR_CMD=("./target/release/calcit")
else
  CR_CMD=(cargo run --bin calcit --)
fi

QUIET_ARGS=()
if [[ "${CI:-}" == "true" ]]; then
  QUIET_ARGS+=(--quiet)
fi

JOBS="${CALCIT_DOCS_CHECK_JOBS:-4}"
# Bound the value before it reaches the arithmetic comparison: the full numeric
# regex alone would accept strings far beyond Bash's integer range, and those
# can wrap in `(( ... ))` instead of failing.
if ! [[ "$JOBS" =~ ^[1-9][0-9]{0,2}$ ]] || (( JOBS > 64 )); then
  echo "CALCIT_DOCS_CHECK_JOBS must be an integer between 1 and 64, got '$JOBS'" >&2
  exit 1
fi

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/calcit-docs-check.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT

# Runs one file and records its exit code and captured output. Always exits 0
# so the concurrency limiter can use `wait` without tripping `set -e`.
check_one() {
  local index="$1"
  local file="$2"
  local output=""
  local exit_code=0
  if output="$("${CR_CMD[@]}" "$ENTRY" --compat-types docs check-md "$file" --entry "$ENTRY" "${QUIET_ARGS[@]}" 2>&1)"; then
    :
  else
    exit_code=$?
  fi
  printf '%s\n' "$exit_code" >"$WORK_DIR/${index}.status"
  printf '%s' "$output" >"$WORK_DIR/${index}.out"
}

FILES=()
while IFS= read -r file; do
  FILES+=("$file")
done < <(find docs -name '*.md' | sort)

PIDS=()
for index in "${!FILES[@]}"; do
  check_one "$index" "${FILES[$index]}" &
  PIDS+=("$!")

  while (( ${#PIDS[@]} >= JOBS )); do
    wait "${PIDS[0]}"
    PIDS=("${PIDS[@]:1}")
  done
done

if (( ${#PIDS[@]} > 0 )); then
  wait
fi

FAILED=0
TOTAL_BLOCKS=0
FAILED_BLOCKS=0

for index in "${!FILES[@]}"; do
  output="$(cat "$WORK_DIR/${index}.out")"
  exit_code="$(cat "$WORK_DIR/${index}.status")"

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
done

TOTAL=${#FILES[@]}

echo ""
echo "Docs check-md: ${TOTAL} files, $((TOTAL - FAILED)) passed, ${FAILED} failed; ${TOTAL_BLOCKS} blocks, $((TOTAL_BLOCKS - FAILED_BLOCKS)) passed, ${FAILED_BLOCKS} failed"

if [ "${FAILED}" -gt 0 ] || [ "${FAILED_BLOCKS}" -gt 0 ]; then
  exit 1
fi
