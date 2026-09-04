# Skip static Dynamic boundary bindings

## Context

Review of PR #639 noted that strict boundary checking cloned the generic
binding map and walked every non-Dynamic argument, including contracts that
cannot bind any type variables.

## Changes

- Skip the transactional generic-binding pass when the expected argument type
  contains no type variables.
- Preserve the existing transactional behavior for generic contracts, where a
  failed match must not leak partial bindings.

## Validation

- Formatting, clippy, focused tests, serial Rust tests, and repository gates
  are rerun before push.
