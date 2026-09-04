# Preserve generic Struct arguments across type-slot bindings

- Reused one Struct-value annotation helper from ordinary static inference and
  `with-type-slot`, preventing generic Struct instances from degrading to an
  argument-free `StructValue` annotation.
- Added a regression showing that a `Struct<T>` contract still detects a
  nested Dynamic applied argument after the type-slot conversion path.
- Strengthened the CLI fixture assertions for both rendered actual and expected
  type fragments in `E_ERASED_GENERIC_RELATION`.
- Verified the review fix with Clippy, focused tests, full `cargo test`, and
  `yarn check-all`.
