# Preserve interleave member types

## Context

The bundled-core Dynamic classification marked `calcit.core/interleave` as a
caller-visible migration target. Its two inputs had unrelated element type
variables while the bare `List` return erased the result member type. The
language has no union type that could honestly represent arbitrary alternating
members.

## Changes

- Model `interleave` as `Fn<T>(List<T>, List<T>) -> List<T>` and carry the same
  relation through its recursive helper.
- Keep heterogeneous use explicit: callers must normalize both inputs or
  declare `List<Dynamic>` at a reviewed open boundary.
- Make ordinary function argument binding transactional and substitute existing
  bindings before matching later arguments, so diagnostics show concrete types
  and failed nested matches cannot contaminate later arguments.
- Add positive core coverage, negative fixture coverage, migration guidance,
  documentation, and refreshed quality/classification artifacts.

## Evidence

- The core definition moves from partial to full type coverage.
- Core quality changes from `schemaDynamic 278 / typeNotFull 134 / unresolved
  184` to `277 / 133 / 183`.
- Cross-workspace search found no executable dependency use of core
  `interleave`; the historical heterogeneous calls are core/caltrop examples,
  while similarly named parser helpers are unrelated definitions.

## Validation

- Focused positive and negative tests pass before full repository validation.
- Full formatting, clippy, Rust, core, JavaScript, IR, WASM, Agent-interface,
  quality, and classification gates are rerun before push.
