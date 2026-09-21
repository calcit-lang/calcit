#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
calcit_bin="${CALCIT_BIN:-calcit}"
bindgen_bin="${CALCIT_BINDGEN_BIN:-calcit-bindgen}"
host_root="$repo_root/examples/wasi-http-client/host"
request_file="$host_root/input/request.local.cirru"
request_example="$host_root/input/request.example.cirru"
capability_file="$host_root/capabilities.local.cirru"
capability_example="$host_root/capabilities.example.cirru"
result_dir="$host_root/output"
result_file="$result_dir/result.cirru"
host_manifest="$host_root/generated-component/rust/wasmtime-http-host/Cargo.toml"
server_log="$(mktemp)"
host_stdout="$(mktemp)"
host_stderr="$(mktemp)"
server_pid=""

cleanup() {
  if [[ -n "$server_pid" ]]; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  rm -f "$server_log" "$host_stdout" "$host_stderr" "$request_file" "$capability_file"
  rm -f "$result_file"
  rmdir "$result_dir" 2>/dev/null || true
}

run_host_case() {
  local expected_status="$1"
  local expected_result="$2"
  local expected_stderr="${3:-}"
  rm -f "$result_file"
  set +e
  cargo run --quiet --manifest-path "$host_manifest" -- "$capability_file" \
    > "$host_stdout" 2> "$host_stderr"
  local actual_status=$?
  set -e
  if [[ "$actual_status" -ne "$expected_status" ]]; then
    echo "expected host exit $expected_status, got $actual_status" >&2
    cat "$host_stdout" >&2
    cat "$host_stderr" >&2
    return 1
  fi
  if [[ -n "$expected_result" ]]; then
    test -f "$result_file"
    grep -q -- "$expected_result" "$result_file"
  else
    test ! -e "$result_file"
  fi
  if [[ -n "$expected_stderr" ]]; then
    grep -q -- "$expected_stderr" "$host_stderr"
  fi
}

cd "$repo_root"
started_at="$(date +%s)"

for local_path in "$request_file" "$capability_file" "$result_dir"; do
  if [[ -e "$local_path" ]]; then
    echo "refusing to replace existing local smoke path: $local_path" >&2
    exit 1
  fi
done
trap cleanup EXIT

echo "## Published toolchain"
"$calcit_bin" -v
"$bindgen_bin" --version

"$calcit_bin" examples/wasi-http-client/calcit.cirru test --tag wasm --require-match
"$calcit_bin" examples/wasi-http-client/calcit.cirru \
  wasm --boundary component \
  --emit-path examples/wasi-http-client/host/generated-core
"$calcit_bin" examples/wasi-http-client/calcit.cirru \
  ffi export --boundary component --format edn \
  > examples/wasi-http-client/host/interface.cirru
"$bindgen_bin" generate \
  examples/wasi-http-client/host/interface.cirru \
  --core-module examples/wasi-http-client/host/generated-core/program.wasm \
  --out examples/wasi-http-client/host/generated-component
"$bindgen_bin" check \
  examples/wasi-http-client/host/interface.cirru \
  --core-module examples/wasi-http-client/host/generated-core/program.wasm \
  --out examples/wasi-http-client/host/generated-component

mkdir -p "$result_dir"
cp "$request_example" "$request_file"
cp "$capability_example" "$capability_file"
python3 -m http.server 8123 --bind 127.0.0.1 >"$server_log" 2>&1 &
server_pid=$!
for _ in $(seq 1 50); do
  if curl --fail --silent --output /dev/null http://127.0.0.1:8123/README.md; then
    break
  fi
  sleep 0.1
done
curl --fail --silent --output /dev/null http://127.0.0.1:8123/README.md

echo "## Business outcomes"
run_host_case 0 "'ok"

sed 's/:allowed-origins $ \[\] |http:\/\/127.0.0.1:8123/:allowed-origins $ []/' \
  "$capability_example" > "$capability_file"
run_host_case 3 "capability-denied"

cp "$capability_example" "$capability_file"
sed 's/65536/8/g' "$request_example" > "$request_file"
sed -i.bak 's/65536/8/g' "$capability_file"
rm -f "$capability_file.bak"
run_host_case 5 "response-too-large"

sed 's/8123/1/g' "$request_example" > "$request_file"
sed 's/8123/1/g' "$capability_example" > "$capability_file"
run_host_case 4 "transport"

printf '{} (:invalid true)\n' > "$request_file"
cp "$capability_example" "$capability_file"
run_host_case 2 "" "must contain one top-level list"

cp "$request_example" "$request_file"
sed 's/:access :read-write/:access :read/' "$capability_example" > "$capability_file"
run_host_case 3 ""

"$bindgen_bin" check \
  examples/wasi-http-client/host/interface.cirru \
  --core-module examples/wasi-http-client/host/generated-core/program.wasm \
  --out examples/wasi-http-client/host/generated-component

component_bytes="$(wc -c < "$host_root/generated-component/component/component.wasm" | tr -d ' ')"
elapsed="$(( $(date +%s) - started_at ))"
echo "- elapsed-seconds: $elapsed"
echo "- component-bytes: $component_bytes"
echo "- outcomes: success, invalid-input, network-denied, output-denied, transport, response-too-large"
