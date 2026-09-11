# Resolve expression-level trait references before JS lowering

- `unsafe-coerce` and `assert-type` synthesize their result from an expression-level type form. A qualified trait name previously remained a generic `TypeRef`, so external-object method calls fell through to dynamic dispatch even though the trait source and FFI metadata were available.
- Qualified, zero-argument `TypeRef` values are now checked against source-backed `deftrait` declarations during expression type synthesis. Successful lookups retain the exact `namespace/definition` provenance used by external-object metadata and JavaScript name mappings.
- Unknown references and non-trait definitions remain ordinary `TypeRef` values; the compiler does not invent a trait or broaden dynamic method specialization.
- Real Respo validation confirms `respo.dom/DomCanvasElement.get-context` lowers to the mapped JavaScript `getContext` call and succeeds with a mock Canvas runtime.
