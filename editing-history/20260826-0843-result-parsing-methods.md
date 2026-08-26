# Result-returning parsing methods

- Added `.parse-cirru`, `.parse-cirru-list`, `.parse-cirru-edn`, and `.parse-json` to the core String method bag. Their function-form compatibility entry points are `try-parse-*`.
- Recoverable parse failures now return nominal `Result` values instead of requiring user code to catch exceptions or interpret nil. CirruQuote/List successes retain their known outer shape; open EDN/JSON payloads deliberately remain Dynamic.
- Moved detailed Cirru parser diagnostics into `CalcitErr.msg` and removed unconditional stderr printing, so a caught failure retains line/column/nearby-input context.
- Added definition-attached native tests, native/JS examples, and EDN documentation. Full Calcit gates and latest Respo/Recollect native, JS, docs, and WASM regressions passed after refreshing locked modules and toolchains.
- A separate narrow typed Macro IR experiment reduced the measured Respo macro-evaluator phase by about 12% but did not produce a stable end-to-end check improvement; it was not landed and the evidence is recorded on issue #436.
