#!/usr/bin/env bash
# Verify the shared Calcit command fixture through the JavaScript backend.
set -euo pipefail

readonly CALCIT_BIN="${CALCIT_BIN:-target/debug/calcit}"
readonly FIXTURE="calcit/test-wasi-command.cirru"
readonly OUTPUT_DIR="target/js-command-smoke"
readonly STDOUT_FILE="$OUTPUT_DIR/stdout.txt"
readonly STDERR_FILE="$OUTPUT_DIR/stderr.txt"
readonly INVALID_EXIT_STDERR="$OUTPUT_DIR/invalid-exit-stderr.txt"

"$CALCIT_BIN" --emit-path "$OUTPUT_DIR" "$FIXTURE" js
CALCIT_WASI_TEST_ENV=环境 node --input-type=module --eval \
  'import("./target/js-command-smoke/app.main.mjs").then((module) => module.main_$x_())' \
  alpha "参数" >"$STDOUT_FILE" 2>"$STDERR_FILE"
grep -Fxq "WASI-stdout: 你好" "$STDOUT_FILE"
grep -Fxq "WASI-env: 环境" "$STDOUT_FILE"
grep -Fxq "WASI-arg: alpha" "$STDOUT_FILE"
grep -Fxq "WASI-arg: 参数" "$STDOUT_FILE"
grep -Fxq "WASI-stderr: 42" "$STDERR_FILE"

"$CALCIT_BIN" --init-fn app.main/exit-7! --emit-path "$OUTPUT_DIR" "$FIXTURE" js
js_exit_status=0
node --input-type=module --eval \
  'import("./target/js-command-smoke/app.main.mjs").then((module) => module.exit_7_$x_())' \
  >/dev/null 2>&1 || js_exit_status=$?
if [[ "$js_exit_status" -ne 7 ]]; then
  echo "JavaScript quit! returned status $js_exit_status instead of 7" >&2
  exit 1
fi

"$CALCIT_BIN" --init-fn app.main/exit-invalid! --emit-path "$OUTPUT_DIR" "$FIXTURE" js
if node --input-type=module --eval \
  'import("./target/js-command-smoke/app.main.mjs").then((module) => module.exit_invalid_$x_())' \
  >/dev/null 2>"$INVALID_EXIT_STDERR"; then
  echo "JavaScript quit! unexpectedly accepted exit status 256" >&2
  exit 1
fi
grep -Fq "integer exit code in 0..255" "$INVALID_EXIT_STDERR"

"$CALCIT_BIN" --init-fn app.main/random-main! --emit-path "$OUTPUT_DIR" "$FIXTURE" js
node --input-type=module --eval \
  'import("./target/js-command-smoke/app.main.mjs").then((module) => module.random_main_$x_())' \
  >"$STDOUT_FILE" 2>"$STDERR_FILE"
grep -Fxq "secure-random: ok" "$STDOUT_FILE"
