#!/usr/bin/env bash
# Verify WASM codegen: generate binary .wasm and validate with Node.js.
# Usage: bash scripts/test-wasm.sh
# Set CR_BIN to override the cr binary path (default: release then debug build).
set -euo pipefail

if [[ -n "${CR_BIN:-}" ]]; then
  BIN="$CR_BIN"
elif [[ -x ./target/release/cr ]]; then
  BIN="./target/release/cr"
elif [[ -x ./target/debug/cr ]]; then
  BIN="./target/debug/cr"
else
  echo "ERROR: cr binary not found. Build first or set CR_BIN."
  exit 1
fi
ENTRY="calcit/test-wasm.cirru"

# Step 1: generate .wasm binary
"$BIN" "$ENTRY" wasm 2>&1

# Step 2: validate and run with Node.js
node scripts/test-wasm.mjs
