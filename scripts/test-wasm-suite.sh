#!/usr/bin/env bash
# Progressive WASM test suite: compile each eligible test file and run main!().
# Usage: bash scripts/test-wasm-suite.sh
# Set CR_WASM_BIN to override the cr-wasm binary path.
set -euo pipefail

if [[ -n "${CR_WASM_BIN:-}" ]]; then
  BIN="$CR_WASM_BIN"
elif [[ -x ./target/release/cr-wasm ]]; then
  BIN="./target/release/cr-wasm"
elif [[ -x ./target/debug/cr-wasm ]]; then
  BIN="./target/debug/cr-wasm"
else
  # Fall back to cargo build
  bash scripts/cargo-with-sdk.sh build --bin cr-wasm --release 2>&1
  BIN="./target/release/cr-wasm"
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
  calcit/test-tuple.cirru
)

pass=0
fail=0
skip=0
total=${#TEST_FILES[@]}

for f in "${TEST_FILES[@]}"; do
  label=$(basename "$f" .cirru)

  # Compile to WASM — allow "skipping" warnings, fail only on real errors
  compile_out=$("$BIN" "$f" 2>&1)
  compile_exit=$?
  if [[ $compile_exit -ne 0 ]] || echo "$compile_out" | grep -qE "^error|thread.*panicked"; then
    echo "  [$label] BUILD-FAIL"
    echo "$compile_out" | grep -E "^error|panicked" | head -3
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
echo "=== WASM suite: $pass/$total passed, $fail failed ==="

[[ $fail -eq 0 ]]
