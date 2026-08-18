# `defimpl` trait arguments use raw symbols

- Migrated ordinary test fixtures from tag-based `defimpl` trait arguments to named `deftrait` values referenced directly as symbols.
- Kept the new trait definitions' entry schemas as `'Trait`, so the snapshot metadata matches their runtime role.
- Updated the `defimpl` macro documentation and diagnostics: raw symbols are the standard syntax; tag arguments remain only as legacy inherent-method-bag compatibility.
- Updated the tuple test to assert the new nominal trait origin rather than the old tag representation.

`&impl::new :name` bootstrap method bags are intentionally unchanged: unlike the macro form, a raw symbol there would be evaluated rather than captured as syntax.
