#!/usr/bin/env bash
# Extended WASM suite: compile calcit/test-wasm-suite.cirru — a hand-authored
# multi-module entry that pulls in pure-compute test modules from yarn check-all
# (test-cond, test-math, test-set, test-tuple, plus util) and runs each
# module's main! in sequence under one WASM instance.
#
# Goal: gradually grow this entry as more procs / syntax are supported in WASM.
#
# Usage: bash scripts/test-wasm-suite-extended.sh
# Set CR_WASM_BIN to override the cr-wasm binary path.
set -euo pipefail

if [[ -n "${CR_WASM_BIN:-}" ]]; then
  BIN="$CR_WASM_BIN"
elif [[ -x ./target/release/cr-wasm ]]; then
  BIN="./target/release/cr-wasm"
elif [[ -x ./target/debug/cr-wasm ]]; then
  BIN="./target/debug/cr-wasm"
else
  bash scripts/cargo-with-sdk.sh build --bin cr-wasm --release 2>&1
  BIN="./target/release/cr-wasm"
fi

ENTRY="calcit/test-wasm-suite.cirru"

echo "[extended] compiling $ENTRY"
"$BIN" "$ENTRY" 2>&1 | grep -E "skipping|wrote|panicked|preprocessing failed" || true

echo ""
echo "[extended] running main!"
node scripts/test-wasm-run.mjs test-wasm-suite
