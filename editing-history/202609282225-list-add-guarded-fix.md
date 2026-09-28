# Guarded List `.add` migration

## Decision

`List.add` is an element append alias, while the `Add` trait has a distinct List-combination implementation. A global rename would conflate these operations. The `core-list-add-v1` rule therefore uses the existing `calcit fix` entrypoint and only rewrites a one-argument method call when the receiver is a concrete `List<T>` and both `.add` and `.append` have proven, identical contracts pointing to `calcit.core/append`.

## Safety boundary

The rule only changes the method leaf, preserving receiver and argument order and evaluation count. It skips quoted data, other known collection types, and method values. Unknown receiver evidence, open List contracts, or an unrecognized enclosing macro remain review-only. This first slice covers definition `:code`; attached tests and examples are not silently rewritten. Preview, revision guard, staged validation, and idempotence reuse the existing fix transaction.

## Validation

CLI integration tests cover a proven List rewrite, retained Calcit behavior, stale revision rejection, idempotence, Set exclusion, quoted data, and an unknown macro boundary. Run `cargo test --test fix_cli list_add_fix`, the full Rust suite, `yarn check-all`, and executable documentation checks before merging.
