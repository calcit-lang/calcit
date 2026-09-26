#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
readonly CALCIT_BIN="$PWD/target/debug/calcit"
readonly SNAPSHOT="$PWD/examples/wasi-command/calcit.cirru"
readonly FIXTURES="$PWD/examples/wasi-command"
readonly OUTPUT="$PWD/target/wasi-manifest-business"
readonly CASE_ROOT=$(mktemp -d "$PWD/target/wasi-manifest.XXXXXX")
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

expect_output() {
  cmp "$FIXTURES/manifest-output.cirru" "$CASE_ROOT/$1/workspace/output.cirru"
  grep -Fxq 'Manifest-written' "$CASE_ROOT/stdout"
}

expect_method_eval_output() {
  local markers
  markers=$(grep -Ex 'receiver|argument|api!' "$1")
  [[ "$markers" == $'receiver\nargument\napi!' ]]
}

run_native() {
  (cd "$1" && "$CALCIT_BIN" --init-fn app.main/manifest-main! "$SNAPSHOT")
}

"$CALCIT_BIN" "$SNAPSHOT" test app.main/process-manifest --require-match
"$CALCIT_BIN" --init-fn app.main/manifest-main! "$SNAPSHOT" --check-only

prepare_case native input
run_native "$CASE_ROOT/native" >"$CASE_ROOT/stdout"
expect_output native
prepare_case native-invalid invalid
expect_status 65 run_native "$CASE_ROOT/native-invalid"
test ! -e "$CASE_ROOT/native-invalid/workspace/output.cirru"
prepare_case native-missing missing
expect_status 66 run_native "$CASE_ROOT/native-missing"
test ! -e "$CASE_ROOT/native-missing/workspace/output.cirru"
prepare_case native-output-denied input
mkdir "$CASE_ROOT/native-output-denied/workspace/output.cirru"
expect_status 73 run_native "$CASE_ROOT/native-output-denied"

mkdir -p "$OUTPUT/js"
"$CALCIT_BIN" --init-fn app.main/manifest-main! --emit-path "$OUTPUT/js" "$SNAPSHOT" js
readonly JS_MODULE="$OUTPUT/js/app.main.mjs"
prepare_case js input
expect_status 0 node scripts/run-wasi-manifest-js.mjs "$JS_MODULE" "$CASE_ROOT/js"
expect_output js
prepare_case js-invalid invalid
expect_status 65 node scripts/run-wasi-manifest-js.mjs "$JS_MODULE" "$CASE_ROOT/js-invalid"
test ! -e "$CASE_ROOT/js-invalid/workspace/output.cirru"
prepare_case js-missing missing
expect_status 66 node scripts/run-wasi-manifest-js.mjs "$JS_MODULE" "$CASE_ROOT/js-missing"
test ! -e "$CASE_ROOT/js-missing/workspace/output.cirru"
prepare_case js-output-denied input
expect_status 73 node scripts/run-wasi-manifest-js.mjs "$JS_MODULE" "$CASE_ROOT/js-output-denied" deny-output
test ! -e "$CASE_ROOT/js-output-denied/workspace/output.cirru"

"$CALCIT_BIN" --init-fn app.main/manifest-main! wasi --boundary native "$SNAPSHOT" --check-only --emit-path "$OUTPUT/preview1-check"
test ! -e "$OUTPUT/preview1-check/program.wasm"
"$CALCIT_BIN" --init-fn app.main/manifest-main! wasi --boundary native "$SNAPSHOT" --emit-path "$OUTPUT/preview1"
readonly WASM_MODULE="$OUTPUT/preview1/program.wasm"
prepare_case preview1 input
expect_status 0 wasmtime run --dir "$CASE_ROOT/preview1/workspace::/workspace" "$WASM_MODULE"
expect_output preview1
prepare_case preview1-invalid invalid
expect_status 65 wasmtime run --dir "$CASE_ROOT/preview1-invalid/workspace::/workspace" "$WASM_MODULE"
test ! -e "$CASE_ROOT/preview1-invalid/workspace/output.cirru"
prepare_case preview1-missing missing
expect_status 66 wasmtime run --dir "$CASE_ROOT/preview1-missing/workspace::/workspace" "$WASM_MODULE"
test ! -e "$CASE_ROOT/preview1-missing/workspace/output.cirru"
prepare_case preview1-output-denied input
mkdir "$CASE_ROOT/preview1-output-denied/workspace/output.cirru"
expect_status 73 wasmtime run --dir "$CASE_ROOT/preview1-output-denied/workspace::/workspace" "$WASM_MODULE"

"$CALCIT_BIN" --init-fn app.main/manifest-main! wasi "$SNAPSHOT" \
  --boundary component --check-only --emit-path "$OUTPUT/component"
test ! -e "$OUTPUT/component/program.wasm"

"$CALCIT_BIN" --init-fn app.main/method-eval-main! "$SNAPSHOT" >"$CASE_ROOT/native-method-eval"
expect_method_eval_output "$CASE_ROOT/native-method-eval"
"$CALCIT_BIN" --init-fn app.main/method-eval-main! --emit-path "$OUTPUT/js-method-eval" "$SNAPSHOT" js
node scripts/run-wasi-manifest-js.mjs "$OUTPUT/js-method-eval/app.main.mjs" "$CASE_ROOT" "" method-eval >"$CASE_ROOT/js-method-eval"
expect_method_eval_output "$CASE_ROOT/js-method-eval"
