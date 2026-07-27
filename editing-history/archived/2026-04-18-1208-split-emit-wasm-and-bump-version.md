## Summary

- Split the WASM code generator so `src/codegen/emit_wasm.rs` keeps orchestration while runtime helpers, method dispatch, and record/tuple emitters live in dedicated submodules.
- Bumped the Calcit patch version from `0.12.20` to `0.12.21` for the next release.

## Module Layout

- `src/codegen/emit_wasm/runtime.rs` now owns host import definitions, module assembly, and internal runtime helper builders.
- `src/codegen/emit_wasm/methods.rs` now owns dynamic method dispatch and rest-arg call argument packing.
- `src/codegen/emit_wasm/records.rs` now owns record and tuple emission helpers.

## Notes

- The split is intended to be behavior-preserving; no WASM semantics were changed as part of this refactor.
- Version strings were updated in `Cargo.toml`, `package.json`, and `lib/package.json`, while generated JS version text will be refreshed by the validation build.