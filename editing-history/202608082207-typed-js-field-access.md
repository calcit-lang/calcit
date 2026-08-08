# Typed `js-get` and `js-set` fields

- Added external-object-aware inference for static `js-get` field reads.
- Added declared field validation for static `js-set` writes, including unknown-field and value-type diagnostics.
- Lowered typed static reads and writes through dedicated AST method kinds so JavaScript codegen applies external trait `:names` overrides and default kebab-to-camel mapping.
- Kept dynamic keys and `aget`/`aset` as explicit raw JavaScript escape hatches.
- Migrated representative Respo DOM field writes and methods to the typed external-object path and verified add/remove behavior in a controlled browser.

Validation:

- `cargo test -q`
- `cargo clippy --lib --bin cr -- -D warnings`
- `yarn compile`
- `cr docs check-md docs/features/js-interop.md --entry calcit/test.cirru`
- Respo: `cr js`, `yarn test`, controlled Chrome add/remove regression
