#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
readonly CALCIT_BIN="$PWD/target/debug/calcit"
readonly SNAPSHOT="$PWD/examples/wasi-command/calcit.cirru"
readonly FIXTURES="$PWD/examples/wasi-command"
readonly WASMTIME_BIN="${WASMTIME_CLI:-wasmtime}"
readonly CASE_ROOT=$(mktemp -d "$PWD/target/wasi03-manifest.XXXXXX")
trap 'rm -rf "$CASE_ROOT"' EXIT

prepare_case() {
  local case_dir="$CASE_ROOT/$1"
  mkdir -p "$case_dir/workspace"
  if [[ "$2" != "missing" ]]; then
    cp "$FIXTURES/manifest-$2.cirru" "$case_dir/workspace/input.cirru"
  fi
}

expect_status() {
  local expected="$1"
  shift
  local actual=0
  "$@" >"$CASE_ROOT/stdout" 2>"$CASE_ROOT/stderr" || actual=$?
  if [[ "$actual" -ne "$expected" ]]; then
    echo "expected exit $expected, got $actual: $*" >&2
    sed -n '1,20p' "$CASE_ROOT/stderr" >&2
    exit 1
  fi
}

run_component() {
  "$WASMTIME_BIN" run -S p3 \
    -W component-model-async-stackful=y \
    -W component-model-more-async-builtins=y \
    --dir "$CASE_ROOT/$1/workspace::/workspace" \
    "$CASE_ROOT/component/program.wasm"
}

"$CALCIT_BIN" --init-fn app.main/manifest-main! wasi "$SNAPSHOT" \
  --boundary component --check-only --emit-path "$CASE_ROOT/check"
test ! -e "$CASE_ROOT/check/program.wasm"
"$CALCIT_BIN" --init-fn app.main/manifest-main! wasi "$SNAPSHOT" \
  --boundary component --emit-path "$CASE_ROOT/component"

prepare_case valid input
expect_status 0 run_component valid
cmp "$FIXTURES/manifest-output.cirru" "$CASE_ROOT/valid/workspace/output.cirru"
grep -Fxq 'Manifest-written' "$CASE_ROOT/stdout"

prepare_case invalid invalid
expect_status 65 run_component invalid
test ! -e "$CASE_ROOT/invalid/workspace/output.cirru"

prepare_case missing missing
expect_status 66 run_component missing
test ! -e "$CASE_ROOT/missing/workspace/output.cirru"

prepare_case output-denied input
mkdir "$CASE_ROOT/output-denied/workspace/output.cirru"
expect_status 73 run_component output-denied

prepare_case no-preopen input
expect_status 66 "$WASMTIME_BIN" run -S p3 \
  -W component-model-async-stackful=y \
  -W component-model-more-async-builtins=y \
  "$CASE_ROOT/component/program.wasm"
test ! -e "$CASE_ROOT/no-preopen/workspace/output.cirru"
