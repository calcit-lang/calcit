# PR review follow-ups for typed FFI

- Fixed JS external-property alias lookup for `JsNullish<TraitSet>` receivers.
- Preprocess prefix typed tag-access receivers before embedding them in external-access IR.
- Resolve external-object trait metadata from the declared `deftrait` name when suppressing legacy member-tag warnings.
- Kept `cirru.edn` fences byte-preserved by markdown formatting and allowed standalone EDN checks without a Calcit entry; failure results now retain the fence mode.
- Clarified the RFC's persisted `:ffi` representation, normalized WASM direction metadata, settled MVP decisions, and repaired the WASM validation document reference.
