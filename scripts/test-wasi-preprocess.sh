#!/usr/bin/env bash
# Reproduce the legacy snapshot preprocessing path in both native and WASI CLIs.
set -euo pipefail

readonly TARGET="wasm32-wasip1"
readonly FIXTURE="calcit/test-wasi-preprocess.cirru"
readonly WASM_BIN="${CARGO_TARGET_DIR:-target}/${TARGET}/debug/cr-wasm.wasm"

command -v wasmtime >/dev/null

# The captured 0.13.77-era snapshot intentionally retains whole-Dynamic schemas.
cargo run --bin calcit -- --compat-types --check-only "$FIXTURE"
cargo build --bin cr-wasm --target "$TARGET"

WASMTIME_NEW_CLI=0 wasmtime run \
  --dir "$PWD::/workspace" \
  "$WASM_BIN" \
  -- --check-only "/workspace/$FIXTURE"
