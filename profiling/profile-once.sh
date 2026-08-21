#!/usr/bin/env bash

set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: profiling/profile-once.sh <entry.cirru> [profile-summary args...]" >&2
  echo "Example: profiling/profile-once.sh calcit/fibo.cirru --top 20 --include 'calcit::'" >&2
  exit 1
fi

entry="$1"
shift

if [[ ! -f "$entry" ]]; then
  echo "Entry file not found: $entry" >&2
  exit 2
fi

for cmd in xctrace cargo python3; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing required command: $cmd" >&2
    exit 2
  fi
done

mkdir -p .tmp-profiles

entry_name="$(basename "$entry" .cirru)"
stamp="$(date +%Y%m%d-%H%M%S)"
trace_path=".tmp-profiles/${entry_name}-${stamp}.trace"

echo "Recording trace to: $trace_path"
xctrace record \
  --template "Time Profiler" \
  --output "$trace_path" \
  --launch -- \
  cargo run --release --bin calcit -- "$entry"

echo
echo "Summarizing hotspots..."
python3 profiling/profile-summary.py --trace "$trace_path" "$@"

echo
echo "Done. Trace bundle: $trace_path"