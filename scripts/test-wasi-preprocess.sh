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
readonly EXIT_OUT="${CARGO_TARGET_DIR:-target}/wasi-exit-smoke"
readonly INVALID_EXIT_OUT="${CARGO_TARGET_DIR:-target}/wasi-invalid-exit-smoke"
readonly INVALID_EXIT_STDERR="${INVALID_EXIT_OUT}/stderr.txt"
readonly CLOCK_OUT="${CARGO_TARGET_DIR:-target}/wasi-clock-smoke"
readonly CLOCK_STDOUT="${CLOCK_OUT}/stdout.txt"
readonly FIXED_CLOCK_OUT="${CARGO_TARGET_DIR:-target}/wasi-fixed-clock-smoke"
readonly CORE_CLOCK_OUT="${CARGO_TARGET_DIR:-target}/core-clock-reject"
readonly RANDOM_OUT="${CARGO_TARGET_DIR:-target}/wasi-random-smoke"
readonly RANDOM_STDOUT="${RANDOM_OUT}/stdout.txt"
readonly FIXED_RANDOM_OUT="${CARGO_TARGET_DIR:-target}/wasi-fixed-random-smoke"
readonly CORE_RANDOM_OUT="${CARGO_TARGET_DIR:-target}/core-random-reject"
readonly FILESYSTEM_OUT="${CARGO_TARGET_DIR:-target}/wasi-filesystem-smoke"
readonly FILESYSTEM_STDOUT="${FILESYSTEM_OUT}/stdout.txt"
readonly FILESYSTEM_DENIED_OUT="${CARGO_TARGET_DIR:-target}/wasi-filesystem-denied"
readonly FILESYSTEM_DENIED_STDOUT="${FILESYSTEM_DENIED_OUT}/stdout.txt"
readonly FILESYSTEM_TRAVERSAL_OUT="${CARGO_TARGET_DIR:-target}/wasi-filesystem-traversal"
readonly FILESYSTEM_TRAVERSAL_STDOUT="${FILESYSTEM_TRAVERSAL_OUT}/stdout.txt"
readonly FILESYSTEM_UTF8_OUT="${CARGO_TARGET_DIR:-target}/wasi-filesystem-invalid-utf8"
readonly FILESYSTEM_UTF8_STDOUT="${FILESYSTEM_UTF8_OUT}/stdout.txt"
readonly FILESYSTEM_ABSOLUTE_OUT="${CARGO_TARGET_DIR:-target}/wasi-filesystem-absolute"
readonly FILESYSTEM_ABSOLUTE_STDOUT="${FILESYSTEM_ABSOLUTE_OUT}/stdout.txt"
readonly CORE_FILESYSTEM_OUT="${CARGO_TARGET_DIR:-target}/core-filesystem-reject"
readonly CHECK_ONLY_OUT="${CARGO_TARGET_DIR:-target}/wasi-check-only-smoke"
readonly NATIVE_WASM_BIN="${CARGO_TARGET_DIR:-target}/debug/cr-wasm"
readonly CALCIT_BIN="${CARGO_TARGET_DIR:-target}/debug/calcit"
WASI_FS_HOST_DIR=$(mktemp -d "${TMPDIR:-/tmp}/calcit-wasi-fs.XXXXXX")
readonly WASI_FS_HOST_DIR
trap 'rm -rf "$WASI_FS_HOST_DIR"' EXIT

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

native_exit_status=0
"$CALCIT_BIN" --init-fn app.main/exit-7! "$COMMAND_FIXTURE" >/dev/null 2>&1 || native_exit_status=$?
if [[ "$native_exit_status" -ne 7 ]]; then
  echo "native quit! returned status $native_exit_status instead of 7" >&2
  exit 1
fi

if native_invalid_error=$("$CALCIT_BIN" --init-fn app.main/exit-invalid! "$COMMAND_FIXTURE" 2>&1); then
  echo "native quit! unexpectedly accepted exit status 256" >&2
  exit 1
fi
grep -Fq "integer exit code in 0..255" <<<"$native_invalid_error"

"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --init-fn app.main/exit-7! --emit-path "$EXIT_OUT"
wasi_exit_status=0
wasmtime run "$EXIT_OUT/program.wasm" >/dev/null 2>&1 || wasi_exit_status=$?
if [[ "$wasi_exit_status" -ne 7 ]]; then
  echo "WASI quit! returned status $wasi_exit_status instead of 7" >&2
  exit 1
fi


"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --init-fn app.main/exit-invalid! --emit-path "$INVALID_EXIT_OUT"
if wasmtime run "$INVALID_EXIT_OUT/program.wasm" >/dev/null 2>"$INVALID_EXIT_STDERR"; then
  echo "WASI quit! unexpectedly accepted exit status 256" >&2
  exit 1
fi
grep -Fq "unreachable" "$INVALID_EXIT_STDERR"

"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --init-fn app.main/clock-main! --emit-path "$CLOCK_OUT"
wasmtime run "$CLOCK_OUT/program.wasm" >"$CLOCK_STDOUT"
grep -Fxq "WASI-clocks: ok" "$CLOCK_STDOUT"

"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --init-fn app.main/clock-fixed-main! --emit-path "$FIXED_CLOCK_OUT"
node scripts/test-wasi-clock-host.mjs "$FIXED_CLOCK_OUT/program.wasm"

if clock_capability_error=$("$CALCIT_BIN" wasm "$COMMAND_FIXTURE" --init-fn app.main/clock-main! --emit-path "$CORE_CLOCK_OUT" 2>&1); then
  echo "core WASM unexpectedly accepted process clocks" >&2
  exit 1
fi
grep -Fq "E_WASM_CAPABILITY" <<<"$clock_capability_error"

"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --init-fn app.main/random-main! --emit-path "$RANDOM_OUT"
wasmtime run "$RANDOM_OUT/program.wasm" >"$RANDOM_STDOUT"
grep -Fxq "secure-random: ok" "$RANDOM_STDOUT"

"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --init-fn app.main/random-fixed-main! --emit-path "$FIXED_RANDOM_OUT"
node scripts/test-wasi-random-host.mjs "$FIXED_RANDOM_OUT/program.wasm"

if random_capability_error=$("$CALCIT_BIN" wasm "$COMMAND_FIXTURE" --init-fn app.main/random-main! --emit-path "$CORE_RANDOM_OUT" 2>&1); then
  echo "core WASM unexpectedly accepted secure random bytes" >&2
  exit 1
fi
grep -Fq "E_WASM_CAPABILITY" <<<"$random_capability_error"

printf '%s' 'WASI-file: 你好' >"$WASI_FS_HOST_DIR/input.txt"
printf '\377\n' >"$WASI_FS_HOST_DIR/invalid.txt"
"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --init-fn app.main/filesystem-main! --emit-path "$FILESYSTEM_OUT"
wasmtime run \
  --dir "$WASI_FS_HOST_DIR::/workspace" \
  "$FILESYSTEM_OUT/program.wasm" >"$FILESYSTEM_STDOUT"
grep -Fxq "WASI-filesystem: ok" "$FILESYSTEM_STDOUT"
grep -Fxq 'WASI-written: 好' "$WASI_FS_HOST_DIR/output.txt"
node scripts/test-wasi-filesystem-host.mjs "$FILESYSTEM_OUT/program.wasm"

"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --init-fn app.main/filesystem-denied-main! --emit-path "$FILESYSTEM_DENIED_OUT"
wasmtime run "$FILESYSTEM_DENIED_OUT/program.wasm" >"$FILESYSTEM_DENIED_STDOUT"
grep -Fxq "WASI-filesystem-denied: ok" "$FILESYSTEM_DENIED_STDOUT"

"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --init-fn app.main/filesystem-traversal-main! --emit-path "$FILESYSTEM_TRAVERSAL_OUT"
wasmtime run \
  --dir "$WASI_FS_HOST_DIR::/workspace" \
  "$FILESYSTEM_TRAVERSAL_OUT/program.wasm" >"$FILESYSTEM_TRAVERSAL_STDOUT"
grep -Fxq "WASI-filesystem-traversal: ok" "$FILESYSTEM_TRAVERSAL_STDOUT"

"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --init-fn app.main/filesystem-invalid-utf8-main! --emit-path "$FILESYSTEM_UTF8_OUT"
wasmtime run \
  --dir "$WASI_FS_HOST_DIR::/workspace" \
  "$FILESYSTEM_UTF8_OUT/program.wasm" >"$FILESYSTEM_UTF8_STDOUT"
grep -Fxq "WASI-filesystem-invalid-utf8: ok" "$FILESYSTEM_UTF8_STDOUT"

"$CALCIT_BIN" wasi "$COMMAND_FIXTURE" --init-fn app.main/filesystem-absolute-main! --emit-path "$FILESYSTEM_ABSOLUTE_OUT"
wasmtime run \
  --dir "$WASI_FS_HOST_DIR::/workspace" \
  "$FILESYSTEM_ABSOLUTE_OUT/program.wasm" >"$FILESYSTEM_ABSOLUTE_STDOUT"
grep -Fxq "WASI-filesystem-absolute: ok" "$FILESYSTEM_ABSOLUTE_STDOUT"

if filesystem_capability_error=$(
  "$CALCIT_BIN" wasm "$COMMAND_FIXTURE" --init-fn app.main/filesystem-main! --emit-path "$CORE_FILESYSTEM_OUT" 2>&1
); then
  echo "core WASM unexpectedly accepted filesystem effects" >&2
  exit 1
fi
grep -Fq "E_WASM_CAPABILITY" <<<"$filesystem_capability_error"

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
