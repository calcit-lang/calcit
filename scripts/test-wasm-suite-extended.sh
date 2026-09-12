#!/usr/bin/env bash
# Extended WASM suite: compile calcit/test-wasm-suite.cirru — a hand-authored
# multi-module entry that pulls in pure-compute test modules from yarn check-all
# (test-cond, test-math, test-set, test-anonymous-enum, plus util) and runs each
# module's main! in sequence under one WASM instance.
#
# Goal: gradually grow this entry as more procs / syntax are supported in WASM.
#
# Usage: bash scripts/test-wasm-suite-extended.sh
# Set CALCIT_BIN to override the calcit binary path.
set -euo pipefail

if [[ -n "${CALCIT_BIN:-}" ]]; then
  BIN="$CALCIT_BIN"
elif [[ -x ./target/release/calcit ]]; then
  BIN="./target/release/calcit"
elif [[ -x ./target/debug/calcit ]]; then
  BIN="./target/debug/calcit"
else
  bash scripts/cargo-with-sdk.sh build --bin calcit --release 2>&1
  BIN="./target/release/calcit"
fi

ENTRY="calcit/test-wasm-suite.cirru"

echo "[extended] compiling $ENTRY"
"$BIN" --compat-types wasm "$ENTRY" 2>&1 | grep -E "skipping|wrote|panicked|preprocessing failed" || true

echo ""
echo "[extended] running main!"
node scripts/test-wasm-run.mjs test-wasm-suite
