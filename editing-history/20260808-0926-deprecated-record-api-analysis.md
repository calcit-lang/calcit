# Deprecated record API analysis

## Summary

- Marked the retained `record-match`, `record-struct`, `record-with`, and `record?` compatibility APIs in `calcit.core` with `:deprecated` tags.
- Updated each API document to name its Struct-model replacement.
- Added `cr analyze deprecated`, a tag-driven static report that finds calls to any `:deprecated` definition and reports the calling definition, `code@...` path, target documentation, and JSON `W_DEPRECATED_API` diagnostic.
- Added a focused regression test for implicit `calcit.core/record?` call detection.

## Validation

- `cargo test --bin cr deprecated_api::tests::finds_implicit_core_deprecated_call_at_body_path`
- `cargo check --all-targets`
- `cargo fmt --check`
- `git diff --check`
- Manual temporary Snapshot validation: `record?` produced one `calcit.core/record?` report at `code@3` and `W_DEPRECATED_API`.
