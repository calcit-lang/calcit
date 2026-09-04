# Rebase List.apply contracts onto Ref watcher main

## Context

PR #636 landed while the List.apply contract PR was open. Both branches extend
the shared collection type-fail fixture and its Rust assertions, producing a
textual conflict even though the Ref watcher and List.apply cases are
independent.

## Changes

- Rebase the List.apply commits onto current main containing the Ref watcher
  contracts.
- Restore the `.apply`, `&list:apply`, and variadic `.merge` fixture nodes with
  `calcit tree insert-before` after retaining main's Snapshot conflict side.
- Restore the direct and method List.apply diagnostic assertions alongside the
  newly landed watcher assertions.

## Validation

- `cargo test type_fail_collection_member_contract_fixture_reports_warning_codes -- --test-threads=1`
- Full repository gates are rerun before the rewritten branch is pushed.
