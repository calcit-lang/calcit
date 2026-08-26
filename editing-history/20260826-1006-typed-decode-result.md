# Typed decode Result syntax

## Summary

- Added `try-parse-cirru-edn-as text TypeExpr` and `try-decode-map-as value TypeExpr`.
- Decoder graphs are still derived during preprocessing. Invalid or open target types remain compile-time errors.
- Runtime parse and recursive shape failures now become nominal `Result<T,String>` errors with their structural path intact.
- Known non-String text inputs are rejected during preprocessing; Dynamic boundaries retain runtime validation.
- Native and generated JavaScript implement the same Result contract. WASM keeps the existing explicit typed-decoder unsupported boundary.

## Backend consistency

Generated JavaScript `try` handlers previously received a host `Error` object even though native Calcit and the public guide specify a String message. JS codegen now normalizes `Error.message` (or stringifies a non-Error throw) before invoking the handler. The new Result syntaxes use the same normalization, so their `:err` payload is a String on both supported backends.

## Type and performance assessment

The visible benefit is semantic and static: successful expressions infer `Result<T,String>` with the complete target type, malformed input is explicit in control flow, invalid target schemas fail before runtime, and known invalid text arguments are rejected during preprocessing. This slice does not claim a speedup. Decoder work is unchanged and the safe success path additionally constructs one Result value; callers should choose it for recoverable external input rather than throughput.

## Validation

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test` (520 library, 246 CLI, 24 caps, 4 WASM tests)
- `yarn compile`
- `yarn check-all` (native, generated JS, IR, WASM, 228 core tests)
- `yarn check-agent-interface` (17/17)
- Native and generated-JS attached examples for success, failure, exact type, and String error payload
- `docs/data/edn.md` 24/24 and `docs/features/error-handling.md` 9/9 markdown blocks
- Latest Respo main: 27/27 tests and 111/111 documentation blocks
- Latest Recollect main: 9/9 native tests, generated JS, and required WASM checks
