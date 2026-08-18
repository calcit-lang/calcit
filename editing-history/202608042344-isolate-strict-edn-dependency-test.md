# Isolate strict EDN dependency test state

## Knowledge

- Program dependency tests share the global definition-ID index with cache and snapshot tests, so every test that calls `ensure_def_id` must use `PROGRAM_TEST_LOCK` and reset the shared state before asserting IDs.

## Changes

- Added the existing program-test lock and reset protocol to the strict EDN nominal dependency test.

## Validation

- `cargo test`
