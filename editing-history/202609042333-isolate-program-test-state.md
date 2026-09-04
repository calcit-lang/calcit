# Isolate process-wide program state in Rust tests

## Problem

Default-parallel Rust tests used separate module-local mutexes while sharing the
same process-wide program registries. A `program` test could therefore replace
source or compiled data while a `preprocess` or Snapshot schema test resolved a
type through the registered program lookup callbacks. The resulting failures
were order-dependent and ranged from qualified type-reference drift to stack
overflow.

## State inventory

The shared test guard covers all mutable process-wide state owned by
`program.rs`:

- source definitions (`PROGRAM_CODE_DATA`);
- runtime cells (`PROGRAM_RUNTIME_DATA_STATE`);
- compiled definitions (`PROGRAM_COMPILED_DATA_STATE`);
- stable definition IDs (`PROGRAM_DEF_ID_INDEX`);
- active entry feature policy (`ACTIVE_FEATURE_POLICY`);
- active compilation target (`ACTIVE_TARGET`).

Type-slot bindings and warning/import-resolution stacks are thread-local, so
they do not need cross-thread serialization. Preprocess project namespaces
already use their own scoped restoration guard.

## Isolation contract

Tests that read or mutate process-wide program state acquire
`program::lock_program_test_state`. The guard snapshots the registries while
holding one cross-module mutex and restores them in `Drop`, including panic
unwinding. Program and preprocess tests no longer use independent locks, and
the Snapshot schema regressions join the same isolation domain.

Tests that only use local values remain parallel. New tests that install source
definitions, compile into the global registry, or resolve schemas through the
registered lookup callbacks must acquire the shared guard for their full
critical section.

## Verification

- focused panic-restoration, Snapshot schema, and preprocess regressions;
- five consecutive default-parallel `cargo test` runs;
- serial full-suite, formatting, Clippy, and repository integration gates.
