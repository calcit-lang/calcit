#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
cargo rustc \
  --manifest-path wasm-runtime/number-format/Cargo.toml \
  --locked --release --target wasm32-unknown-unknown --lib -- \
  -C link-arg=--import-memory \
  -C link-arg=--global-base=8192 \
  -C link-arg=-zstack-size=2048
cp wasm-runtime/number-format/target/wasm32-unknown-unknown/release/calcit_wasm_number_format.wasm \
  src/codegen/emit_wasm/number-format.wasm
chmod -x src/codegen/emit_wasm/number-format.wasm
