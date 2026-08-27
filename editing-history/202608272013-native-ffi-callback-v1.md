# Native C-safe callback FFI v1

## Goal

Move `&call-dylib-edn-fn` onto the versioned asynchronous C ABI without
breaking modules that still expose the guarded Rust callback ABI.

## Changes

- added the versioned native async start symbol and process-lifetime C host
  function table;
- copied and validated foreign byte payloads before placing them in the
  bounded host queue;
- decoded `Emit` arguments on the CLI host thread, required explicit `&unit`
  completion, and surfaced structured `Fail` diagnostics;
- reclaimed handles and tracking state on terminal delivery, callback failure,
  and start failure;
- kept per-method fallback to the build-identity-guarded Rust ABI while making
  an advertised version mismatch a hard error;
- added ABI, payload, foreign-producer, host-drain, lifecycle, and failure
  regression coverage;
- documented symbols, ownership, payload rules, statuses, and rollout.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -- --test-threads=1`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`
