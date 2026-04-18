#!/usr/bin/env bash
# Verify WASM codegen: generate binary .wasm and validate with Node.js.
# Usage: bash scripts/test-wasm.sh
# Set CR_WASM_BIN to override the cr-wasm binary path (default: release then debug build).
set -euo pipefail

if [[ -n "${CR_WASM_BIN:-}" ]]; then
  BIN="$CR_WASM_BIN"
elif [[ -x ./target/release/cr-wasm ]]; then
  BIN="./target/release/cr-wasm"
elif [[ -x ./target/debug/cr-wasm ]]; then
  BIN="./target/debug/cr-wasm"
else
  BIN=""
fi
ENTRY="calcit/test-wasm.cirru"

# Step 1: generate .wasm binary
if [[ -n "$BIN" ]]; then
  "$BIN" "$ENTRY" 2>&1
else
  bash scripts/cargo-with-sdk.sh run --bin cr-wasm -- "$ENTRY" 2>&1
fi

# Step 2: validate and run with Node.js
node scripts/test-wasm.mjs
