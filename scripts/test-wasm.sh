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
FAIL_ENTRY="calcit/type-fail/schema-required-arity.cirru"

run_codegen() {
  local entry="$1"
  if [[ -n "$BIN" ]]; then
    "$BIN" "$entry"
  else
    bash scripts/cargo-with-sdk.sh run --bin cr-wasm -- "$entry"
  fi
}

# Step 1: generate .wasm binary
run_codegen "$ENTRY" 2>&1

# Step 2: validate and run with Node.js
node scripts/test-wasm.mjs

# Step 3: a preprocessing failure must stop codegen with a non-zero result.
if failure_out=$(run_codegen "$FAIL_ENTRY" 2>&1); then
  echo "WASM codegen unexpectedly accepted $FAIL_ENTRY" >&2
  exit 1
fi

if ! grep -Fq "WASM preprocessing failed for type-fail-schema-required-arity.main/" <<<"$failure_out" ||
  ! grep -Fq "type-fail-schema-required-arity.main/bad-arity" <<<"$failure_out"; then
  echo "WASM preprocessing failure lost its definition context" >&2
  echo "$failure_out" >&2
  exit 1
fi
