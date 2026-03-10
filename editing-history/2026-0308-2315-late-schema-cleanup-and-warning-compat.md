# 2026-03-08 late schema cleanup and warning compatibility

## Summary

- made snapshot schema loading strict for non-nil schemas, so malformed schema data no longer silently degrades to dynamic
- normalized loaded schema maps so legacy string keys and string `:kind` values are converted into canonical tag-based schema maps
- restored generic type variables in `src/cirru/calcit-core.cirru` where a previous migration had incorrectly replaced them with `:symbol`
- cleaned the remaining mistaken `:symbol` placeholders in core collection schemas, keeping only the intentional `symbol?` runtime type check
- fixed `calcit/test-generics.cirru` after a schema-formatting edit moved `assert-type id ...` into the `let` binding section
- kept `ns/def` warning location reporting for embedded core snapshot loading
- stopped legacy fn-schema warnings from firing on malformed nested `:: :fn` schema payloads parsed from `calcit-core.cirru`
- used a conservative fallback to `DynFn` for malformed nested payloads, avoiding false-positive callback type warnings without re-editing Cirru syntax

## Notes

- `yarn check-all` passed after the generic-schema restoration and remaining `:symbol` cleanup passes
- the `test-generics` fix resolves the runtime error `expected binding of a pair`
- nested function schema payloads coming from Cirru snapshot EDN may currently collapse into malformed map payloads like `(:: :fn ({} (nil ...)))` or `(:: :fn ({}))`
- those malformed payloads are lossy, so partially reconstructing arg/return types can introduce new static-analysis warnings
- decoding the embedded binary core snapshot with per-entry owners is necessary to preserve precise warning locations such as `calcit.core/foo`

## Validation

- `cargo test -q malformed_nested_fn_schema`
- `cargo build -q`
- `target/debug/cr calcit/test.cirru -1`
- `yarn check-all`
