# Explicit Unit literal

- Added `&unit` as a distinct runtime `Calcit::Unit` value. It is intentionally
  not a bare `unit` identifier, so existing programs can continue to bind a
  local named `unit`.
- `&unit` has its own equality, hashing, display, parser/data conversion and
  `type-of` result. JavaScript emits `void 0`, whereas legacy `nil` continues
  to emit `null`.
- Unit schemas accept both `&unit` and legacy `nil` to preserve existing source
  compatibility. JSON refuses to encode `&unit` so it cannot silently become
  a nullable application value.
- Updated JS, IR and WASM code generation plus data-shape validation and
  documentation. The parser/codegen tests cover the literal and the backend
  suite passes.
