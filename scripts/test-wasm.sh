#!/usr/bin/env bash
# Verify WASM codegen: generate binary .wasm and validate with Node.js.
# Usage: bash scripts/test-wasm.sh
# Set CALCIT_BIN to override the calcit binary path (default: release then debug build).
set -euo pipefail

if [[ -n "${CALCIT_BIN:-}" ]]; then
  BIN="$CALCIT_BIN"
elif [[ -x ./target/release/calcit ]]; then
  BIN="./target/release/calcit"
elif [[ -x ./target/debug/calcit ]]; then
  BIN="./target/debug/calcit"
else
  BIN=""
fi
ENTRY="calcit/test-wasm.cirru"
FAIL_ENTRY="calcit/type-fail/schema-required-arity.cirru"
LOWERING_FAIL_ENTRY="calcit/test-struct.cirru"

run_codegen() {
  local entry="$1"
  shift
  if [[ -n "$BIN" ]]; then
    "$BIN" --compat-types wasm "$entry" "$@"
  else
    bash scripts/cargo-with-sdk.sh run --bin calcit -- --compat-types wasm "$entry" "$@"
  fi
}

# Step 1: run the user-visible Calcit semantic contract before backend checks.
if [[ -x ./target/debug/calcit ]]; then
  ./target/debug/calcit "$ENTRY" test --tag wasm --require-match
else
  bash scripts/cargo-with-sdk.sh run --bin calcit -- "$ENTRY" test --tag wasm --require-match
fi

# Step 2: generate .wasm binary
run_codegen "$ENTRY" 2>&1

# Step 3: validate and run with Node.js
node scripts/test-wasm.mjs
node scripts/test-wasm-fail-closed.mjs js-out/program.wasm

# Step 4: a preprocessing failure must stop codegen with a non-zero result.
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

# Step 5: target lowering failures must reject the artifact before writing it.
LOWERING_FAIL_OUT=$(mktemp -d "${TMPDIR:-/tmp}/calcit-wasm-fail-closed.XXXXXX")
trap 'rm -rf "$LOWERING_FAIL_OUT"' EXIT
if lowering_failure_out=$(run_codegen "$LOWERING_FAIL_ENTRY" --emit-path "$LOWERING_FAIL_OUT" 2>&1); then
  echo "WASM codegen unexpectedly accepted an unsupported target namespace" >&2
  exit 1
fi

if [[ -e "$LOWERING_FAIL_OUT/program.wasm" ]]; then
  echo "WASM codegen wrote an artifact after target lowering failed" >&2
  exit 1
fi

if ! grep -Fq "[wasm] target function test-struct.main/" <<<"$lowering_failure_out" ||
  ! grep -Fq "is not compilable" <<<"$lowering_failure_out"; then
  echo "WASM target lowering failure lost its definition context" >&2
  echo "$lowering_failure_out" >&2
  exit 1
fi
