#!/usr/bin/env bash

set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: profiling/samply-once.sh <entry.cirru> [--release] [samply-summary args...]" >&2
  echo "Example: profiling/samply-once.sh calcit/fibo.cirru --top 20 --include 'calcit::'" >&2
  echo "Example: profiling/samply-once.sh calcit/fibo.cirru --release --collapse-hash" >&2
  exit 1
fi

entry="$1"
shift

if [[ ! -f "$entry" ]]; then
  echo "Entry file not found: $entry" >&2
  exit 2
fi

mode="debug"
if [[ "${1:-}" == "--release" ]]; then
  mode="release"
  shift
fi

for cmd in samply cargo python3; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing required command: $cmd" >&2
    exit 2
  fi
done

mkdir -p .tmp-profiles

entry_name="$(basename "$entry" .cirru)"
stamp="$(date +%Y%m%d-%H%M%S)"
samply_path=".tmp-profiles/${entry_name}-${mode}-${stamp}.samply"

if [[ "$mode" == "release" ]]; then
  build_args=(build --release --bin calcit)
  binary="target/release/calcit"
else
  build_args=(build --bin calcit)
  binary="target/debug/calcit"
fi

echo "Building binary: $binary"
cargo "${build_args[@]}"

if [[ ! -f "$binary" ]]; then
  echo "Expected binary not found after build: $binary" >&2
  exit 3
fi

echo "Recording samply profile to: $samply_path"
samply record --save-only -o "$samply_path" -- "$binary" "$entry"

binary_arg=(--binary "$binary")

echo
echo "Summarizing hotspots..."
python3 profiling/samply-summary.py --input "$samply_path" "${binary_arg[@]}" "$@"

echo
echo "Done. Samply file: $samply_path"