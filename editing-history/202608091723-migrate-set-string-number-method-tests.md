# Migrate set, string, and number method tests

Move deterministic set, string, and number method assertions from the legacy
`calcit/test-*.cirru` fixtures to definition-attached tests in
`src/cirru/calcit-core.cirru`.

- Cover the internal set and string primitives at their implementation entries,
  including Unicode character indexing and slicing.
- Add the remaining public numeric `empty` and `inc` checks at their definitions.
- Correct `&number:format` metadata to declare its required decimal-place
  argument, which the migrated test made explicit.
- Delete the redundant `test-methods` blocks from the set, math, and string
  fixtures. Keep a compact set method-dispatch smoke check plus the existing
  target-specific, eval, formatting, Unicode, and WASM coverage.

Validation:

- `yarn try-core-tests` (210 passed)
- `target/debug/cr calcit/test-set.cirru`
- `target/debug/cr calcit/test-math.cirru`
- `target/debug/cr calcit/test-string.cirru`
