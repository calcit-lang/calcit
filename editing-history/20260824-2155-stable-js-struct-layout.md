# Stable JavaScript Struct layout

- Align JavaScript Struct, Enum, and implementation metadata ordering with native lexical tag-name ordering.
- Make typed JS Struct reads and updates consume their precomputed field indices while retaining field-tag consistency checks.
- Canonicalize Cirru EDN Struct field/value pairs during parsing so serialized data adopts the same layout.
- Cover reversed tag-registration order and invalid indexed access in the JavaScript runtime regression suite.
- Add `yarn bench-struct-index` to compare the previous tag-lookup codegen path with indexed access plus its schema-drift check.

Focused JS benchmark on Apple arm64 / Node 24.4.1, 5,000,000 reads over a 32-field Struct (three runs): tag lookup median 574.51 ms; indexed access with field-tag validation median 37.56 ms (~15.3x faster). Native indexed Struct access is unchanged by this JS-only runtime/codegen optimization.
