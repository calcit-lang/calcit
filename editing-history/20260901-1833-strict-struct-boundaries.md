# Strict Struct boundaries / 严格 Struct 边界

- Hand-written `&struct:nth` embeds a field index from one nominal Struct layout. It must remain compiler/runtime IR and must not be used in reusable or generic library code.
- Persisted indexed IR is accepted only when the receiver's concrete nominal Struct proves that the embedded field index and tag still match. This avoids false positives for ordinary `(:field value)` lowering while catching generic or stale indexed access.
- Source-level code should use `(:field value)` on a concrete nominal Struct. Generic helpers should expose a typed trait/accessor, or retain a Map boundary when the record shape is intentionally open.
- The runtime field tag remains useful for detecting stale generated code, but it cannot make a hard-coded index polymorphic.
- Regression context: a Respo cursor helper embedded `:states` at index `1`; a consumer `Store` placed the field at a different index, so the JS runtime rejected the otherwise matching field name.
- `--strict-types` combines location-aware untyped JS FFI diagnostics with the zero-baseline quality gate, so new modules cannot silently accumulate Dynamic, nil, legacy Optional, deprecated-call, or unsafe-coerce debt before execution/codegen.
- Intentionally open deep payloads remain possible through a reviewed `analyze quality --baseline` workflow; they are not mislabeled as zero-debt strict code.
- `decode-map-as` and `try-decode-map-as` reject an already nominal Struct during preprocessing with `E_DECODE_MAP_AS_ALREADY_STRUCT`, so accidental double decoding cannot survive until a runtime history or updater path.
