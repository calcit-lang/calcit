# 2026-06-01 00:03 update callback field specialization

## Summary

- specialized `calcit.core/update` call checking so the 3rd callback arg can inherit a known record/struct field type instead of always staying at generic `fn('T) -> 'T`
- fixed record field inference to resolve `TypeRef`-based struct annotations, not only direct `Record`/`Struct` annotations
- aligned native `not` proc signature with calcit-core semantics by accepting dynamic input and returning `:bool`

## Why

- `respo` used precise type refs like `'respo.app.schema/Task`, but `infer_record_field_type` only looked through direct `Record`/`Struct` annotations, so `update task :done? not` could not see the field type
- even after specializing `update`, Rust-side proc metadata still described `not` as `fn(:bool?) -> :bool`, which was stricter and inconsistent with the core schema/docs path

## Validation

- `cargo test specialize_update_uses_record_field_type_for_callback`
- `cargo run --manifest-path /Users/chenyong/repo/calcit-lang/calcit/Cargo.toml --bin cr -- js` from `respo`
- `yarn try-rs`
- `cargo test`