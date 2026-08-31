# FFI Interface IR v2 composite declarations

## Knowledge captured

- Interface IR must carry namespace-qualified `defstruct` and `defenum` declarations instead of asking generators to infer nominal layout from a name.
- Only declarations transitively reachable from an FFI signature participate in the exported document and revision; unrelated application data must not invalidate generated bindings.
- `Option<T>` and `Result<T, E>` are portable type constructors and therefore use explicit IR nodes. Local nominal references resolve only within their namespace or by an explicit qualified ID.
- Callable schemas remain monomorphic. Declaration type parameters may describe reusable local layouts, while unresolved declarations, wrong arity, trait bounds, and non-portable host/resource types fail with path-specific diagnostics before generation.
- A version bump is required when the document shape changes. The v1 schema remains available as a frozen contract, and v1 consumers must fail explicitly on v2 rather than silently interpreting it.

## Implementation summary

- Added Interface IR v2, its JSON Schema, static declaration extraction, reachable-declaration traversal, and deterministic diagnostics/revisions.
- Extended the preview generator to render composite declarations for Rust, TypeScript, and WIT, with explicit rejection of unsupported generic WIT lowering.
- Added composite, collision, reachability, missing-declaration, arity, version-gate, golden, and WIT parser coverage.
- Updated the agent protocol checks and the bilingual FFI Interface IR documentation.

## Validation notes

- Full Rust, Core, JS, IR, WASM, agent-interface, docs, clippy, and preview tests pass.
- Latest `calcit.std` remains fully generator-safe; `calcit-wss` exports five reachable metric/outcome declarations while retaining explicit diagnostics for callback and resource boundaries.
- Warm release startup remains about 30 ms. The arm64 release binary is 9,516,080 bytes, about 1.3% larger than the preceding local main build due to the bundled v2 schema and declaration machinery.
