# FsPath file-effect boundary

## Summary

- Removed `.read-file`, `.read-dir`, and `.write-file` from the built-in String method table.
- Added the nominal, UTF-8-backed `FsPath` type and the explicit `fs:path` constructor.
- Added `FsPath` methods `.read-text`, `.write-text`, `.read-dir`, `.walk-dir`, and `.to-string` through a nominal trait implementation.
- Kept `try-read-file`, `try-read-dir`, `try-write-file` and the raw raising primitives as String-path compatibility APIs.

## Design notes

- `path` remains available for generic AST, data, and query path concepts; the filesystem-specific type is named `FsPath`.
- Calcit uses one immutable path value rather than copying Rust's borrowed `Path` versus owned mutable `PathBuf` split.
- `fs:path` stores the original UTF-8 text without lexical normalization or filesystem access.
- `.read-dir` is non-recursive and `.walk-dir` is recursive, avoiding an `Option<Bool>` mode in the new public method API.
- Text operations are named `.read-text` and `.write-text` so future byte-oriented operations have an unambiguous place.

## Implementation lesson

Keep `defstruct FsPath` directly inside the public `FsPath` definition before `impl-traits`, matching the core `Result` pattern. Hiding the base struct in an intermediate `FsPath0` definition prevents required-field analysis from resolving `:value` through the nominal reference.

For an expression receiver, prefix method syntax such as `.read-text $ fs:path |file` is always explicit. The ergonomic suffix form should use a typed binding such as `source.read-text`; `(fs:path |file) .read-text` is not a general runtime rewrite for arbitrary expression receivers.

## Validation

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `yarn check-all`
- changed Markdown snippets via `calcit docs check-md`
- latest Respo main: 27/27 tests, 111/111 documentation blocks, JS codegen
- latest Recollect main: 9/9 native tests, generated-JS tests, production build, and required WASM checks

