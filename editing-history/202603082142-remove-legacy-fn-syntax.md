# Remove legacy fn syntax

## Summary

- removed legacy quote-wrapped schema normalization from snapshot loading
- rejected legacy quoted generic symbols in schema EDN parsing
- removed legacy positional `fn` type annotation parsing fallback in Rust
- migrated remaining core trait and test annotations to schema-map `:: :fn {}` form
- updated `deftrait` docs to describe the new wrapped schema syntax only

## Knowledge points

- top-level function and macro schemas are now expected in wrapped form: `:: :fn` / `:: :macro`
- schema payload should be a map using keys like `:args`, `:return`, `:generics`, `:rest`
- generic variables in EDN must be stored as plain symbols like `T`, not legacy quoted symbols like `'T`
- removing parser compatibility is safe only after migrating runtime-loaded Cirru sources, otherwise type info silently degrades to dynamic
- `test-generics` is a good regression check because `&inspect-type` should still render `fn<'T>('T) -> 'T`

## Validation

- `cargo fmt`
- `cargo test -q`
- `cargo build -q`
- `yarn check-all`
