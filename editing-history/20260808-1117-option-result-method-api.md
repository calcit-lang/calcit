# Option and Result Method API

- Classified direct `option:*` and `result:*` helpers as `:internal`; they remain compiler-lowering targets and core implementation details.
- Added public `Option` method entries for `.unwrap` and `.fold`, alongside the existing predicates, mapping, fallback, and chaining methods.
- Migrated public Option examples and lens tests to inferred postfix methods when static type evidence exists; dynamic map lookups continue to use explicit `tag-match`.
- Documented inferred Option/Result methods as the public API, including the JavaScript interop guide.
- Verified direct helpers still provide Result method dispatch; current WASM codegen cannot consume a surviving nominal Option method call, so backend-only helper coverage remains direct until preprocessing lowering is extended.
