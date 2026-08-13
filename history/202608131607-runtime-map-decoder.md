# Runtime map decoder for typed boundary data

## Change summary

- Added `decode-map-as value TypeExpr`, a compile-time-derived decoder for an already-evaluated Calcit Map.
- The decoder constructs nominal Struct values recursively, rejects unknown keys and missing required fields, and reports nested failure paths.
- Struct fields declared as `Option<T>` accept raw `T`, preserve an already-wrapped `%some`/`%none`, and become `%none` when the source field is absent.
- `Dynamic` is allowed only as an explicit runtime-boundary leaf for this decoder; the existing text-based `parse-cirru-edn-as` remains a closed-data decoder and continues to reject it.
- Native and JavaScript backends share the graph ABI and tests. WASM continues to reject both typed EDN decoder syntaxes explicitly.

## Knowledge point

Runtime Maps are not Cirru EDN: they do not carry nominal Struct identity and commonly represent optional data by omitted keys. A typed boundary decoder must therefore perform map-to-Struct construction, enforce required fields, and lift omitted or raw optional values into nominal `Option` variants. Reusing the closed EDN decoder without these rules silently permits invalid Structs or forces callers back to ad-hoc dynamic map readers.
