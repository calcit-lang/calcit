#!/usr/bin/env bash
# Reproduce the legacy snapshot preprocessing path in both native and WASI CLIs.
set -euo pipefail

readonly TARGET="wasm32-wasip1"
readonly FIXTURE="calcit/test-wasi-preprocess.cirru"
readonly WASM_BIN="${CARGO_TARGET_DIR:-target}/${TARGET}/debug/cr-wasm.wasm"
readonly COMMAND_FIXTURE="calcit/test-wasi-command.cirru"
readonly COMMAND_OUT="${CARGO_TARGET_DIR:-target}/wasi-command-smoke"
readonly COMMAND_STDOUT="${COMMAND_OUT}/stdout.txt"
readonly COMMAND_STDERR="${COMMAND_OUT}/stderr.txt"
readonly COMMAND_MISSING_STDOUT="${COMMAND_OUT}/missing-env-stdout.txt"
readonly CHECK_ONLY_OUT="${CARGO_TARGET_DIR:-target}/wasi-check-only-smoke"
readonly NATIVE_WASM_BIN="${CARGO_TARGET_DIR:-target}/debug/cr-wasm"
readonly CALCIT_BIN="${CARGO_TARGET_DIR:-target}/debug/calcit"

command -v wasmtime >/dev/null

# The captured 0.13.77-era snapshot intentionally retains whole-Dynamic schemas.
cargo run --bin calcit -- --compat-types --check-only "$FIXTURE"
cargo build --bin cr-wasm --target "$TARGET"

WASMTIME_NEW_CLI=0 wasmtime run \
  --dir "$PWD/calcit::/workspace" \
  "$WASM_BIN" \
  -- --check-only "/workspace/$(basename "$FIXTURE")"

# The generated command module keeps the Preview 1 bridge internal and starts
# through the conventional no-argument `_start` export.
cargo build --bin cr-wasm --bin calcit
cargo run --bin calcit -- "$COMMAND_FIXTURE" test --tag wasi --require-match
"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --check-only --emit-path "$CHECK_ONLY_OUT"
if [[ -e "$CHECK_ONLY_OUT/program.wasm" ]]; then
  echo "WASI check-only unexpectedly wrote a module" >&2
  exit 1
fi
"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --emit-path "$COMMAND_OUT"
wasmtime run \
  --env CALCIT_WASI_TEST_ENV=环境 \
  "$COMMAND_OUT/program.wasm" \
  alpha "参数" >"$COMMAND_STDOUT" 2>"$COMMAND_STDERR"
grep -Fxq "WASI-stdout: 你好" "$COMMAND_STDOUT"
grep -Fxq "WASI-echo" "$COMMAND_STDOUT"
grep -Fxq "WASI-env: 环境" "$COMMAND_STDOUT"
grep -Fxq "WASI-arg: alpha" "$COMMAND_STDOUT"
grep -Fxq "WASI-arg: 参数" "$COMMAND_STDOUT"
grep -Fxq "WASI-stderr: 42" "$COMMAND_STDERR"
[ "$(grep -Fc "WASI-arg: " "$COMMAND_STDOUT")" -eq 3 ]
[ "$(wc -l <"$COMMAND_STDERR")" -eq 1 ]
wasmtime run --env A=x "$COMMAND_OUT/program.wasm" >"$COMMAND_MISSING_STDOUT" 2>/dev/null
grep -Fxq "WASI-env: missing" "$COMMAND_MISSING_STDOUT"

if target_error=$("$NATIVE_WASM_BIN" "$COMMAND_FIXTURE" --target unknown 2>&1); then
  echo "cr-wasm unexpectedly accepted an unknown target" >&2
  exit 1
fi
grep -Fq "E_WASM_TARGET" <<<"$target_error"

if entry_error=$("$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --init-fn app.main/needs-arg --check-only 2>&1); then
  echo "WASI target unexpectedly accepted a command entry with arguments" >&2
  exit 1
fi
grep -Fq "E_WASM_TARGET" <<<"$entry_error"

if capability_error=$("$CALCIT_BIN" --compat-types wasi calcit/test-wasm.cirru --check-only 2>&1); then
  echo "WASI target unexpectedly accepted a custom host import" >&2
  exit 1
fi
grep -Fq "E_WASM_CAPABILITY" <<<"$capability_error"
