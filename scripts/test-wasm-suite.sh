#!/usr/bin/env bash
# Progressive WASM test suite: compile each eligible test file and run main!().
# Usage: bash scripts/test-wasm-suite.sh
# Set CALCIT_BIN to override the calcit binary path.
set -euo pipefail

if [[ -n "${CALCIT_BIN:-}" ]]; then
  BIN="$CALCIT_BIN"
elif [[ -x ./target/release/calcit ]]; then
  BIN="./target/release/calcit"
elif [[ -x ./target/debug/calcit ]]; then
  BIN="./target/debug/calcit"
else
  # Fall back to cargo build
  bash scripts/cargo-with-sdk.sh build --bin calcit --release 2>&1
  BIN="./target/release/calcit"
fi

# Test files to try — pure-computation, no host FFI dependency
TEST_FILES=(
  calcit/test-math.cirru
  calcit/test-set.cirru
  calcit/test-recursion.cirru
  calcit/test-algebra.cirru
  calcit/test-map.cirru
  calcit/test-list.cirru
  calcit/test-cond.cirru
  calcit/test-fn.cirru
  calcit/test-string.cirru
  calcit/test-anonymous-enum.cirru
)

pass=0
fail=0
skip=0
total=${#TEST_FILES[@]}

for f in "${TEST_FILES[@]}"; do
  label=$(basename "$f" .cirru)

  # Compile to WASM and distinguish explicit unsupported targets from harness failures.
  if compile_out=$("$BIN" --compat-types wasm "$f" 2>&1); then
    compile_exit=0
  else
    compile_exit=$?
  fi
  if [[ $compile_exit -ne 0 ]] && grep -Fq "[wasm] target function" <<<"$compile_out"; then
    echo "  [$label] UNSUPPORTED"
    ((skip++)) || true
    continue
  fi
  if [[ $compile_exit -ne 0 ]] || grep -qE "^error|thread.*panicked" <<<"$compile_out"; then
    echo "  [$label] BUILD-FAIL"
    grep -E "^error|panicked|Error:" <<<"$compile_out" | head -3
    ((fail++)) || true
    continue
  fi

  # Run main!
  if node scripts/test-wasm-run.mjs "$label" 2>&1; then
    ((pass++)) || true
  else
    ((fail++)) || true
  fi
done

echo ""
echo "=== WASM suite: $pass passed, $skip unsupported, $fail failed ($total total) ==="

[[ $fail -eq 0 ]]
