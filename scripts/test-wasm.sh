#!/usr/bin/env bash
# Verify WASM codegen: generate WAT from test-wasm.cirru and validate with wasmtime.
# Usage: bash scripts/test-wasm.sh
# Set CR_BIN to override the cr binary path (default: release then debug build).
set -euo pipefail

if [[ -n "${CR_BIN:-}" ]]; then
  BIN="$CR_BIN"
elif [[ -x ./target/release/cr ]]; then
  BIN="./target/release/cr"
elif [[ -x ./target/debug/cr ]]; then
  BIN="./target/debug/cr"
else
  echo "ERROR: cr binary not found. Build first or set CR_BIN."
  exit 1
fi
WAT="js-out/program.wat"
ENTRY="calcit/test-wasm.cirru"

if ! command -v wasmtime &>/dev/null; then
  echo "SKIP: wasmtime not found, skipping WASM verification"
  exit 0
fi

echo "=== WASM codegen test ==="

# Step 1: generate WAT
"$BIN" "$ENTRY" wasm 2>&1

# Step 2: validate WAT compiles
wasmtime compile "$WAT" -o /dev/null
echo "WAT compilation: OK"

# Step 3: run exported functions and check expected values
export WASMTIME_NEW_CLI=0
fail=0

check() {
  local label="$1"; shift
  local expected="$1"; shift
  local func="$1"; shift
  local got
  got=$(wasmtime run --invoke "$func" "$WAT" -- "$@" 2>&1 | tail -1)
  if [ "$got" = "$expected" ]; then
    echo "  $label = $got  OK"
  else
    echo "  $label = $got  FAIL (expected $expected)"
    fail=1
  fi
}

check "fibo(10)"          "89"       fibo 10
check "factorial(10)"     "3628800"  factorial 10
check "add-two(3.5,2.5)"  "6"       add-two 3.5 2.5
check "sum-range(10)"     "55"       sum-range 10
check "test-floor(3.7)"   "3"       test-floor 3.7
check "test-ceil(3.2)"    "4"       test-ceil 3.2
check "test-round(3.5)"   "4"       test-round 3.5
check "test-sqrt(81)"     "9"       test-sqrt 81
check "test-rem(33,4)"    "1"       test-rem 33 4
check "test-compare(1,2)" "-1"      test-compare 1 2
check "test-compare(2,1)" "1"       test-compare 2 1
check "test-compare(1,1)" "0"       test-compare 1 1
check "test-not(0)"        "1"      test-not 0
check "test-not(1)"        "0"      test-not 1
check "test-let-chain(3)" "20"      test-let-chain 3
check "collatz-steps(27)" "111"     collatz-steps 27
check "gcd(48,18)"        "6"       gcd 48 18

if [ "$fail" -ne 0 ]; then
  echo "WASM verification FAILED"
  exit 1
fi

echo "=== All WASM checks passed ==="
