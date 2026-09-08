# Calx checked-index boundary (#908)

- Source `&f64:to-i64-index` returns a validated Calcit Number, while its VM opcode produces I64. Treating the opcode result as a planned F64 lets invalid locals, returns and arithmetic reach VM validation.
- Eligibility now accepts the conversion only as the direct index of a buffer read. Its one operand must remain F64; malformed read shapes and operand types always produce a fallback issue.
- Plan the buffer and original F64 operand, then emit conversion plus read together. There is no standalone conversion plan or I64 source slot; no new Nil, Dynamic, public type, opcode or implicit conversion.
- Source-backed tests cover return, local, arithmetic, branch, recur argument, unchecked read and nested conversion. Existing dot-product/gather golden programs, native parity and runtime trap origins remain unchanged.
- Native-only source forms continue to work; unsupported Calx forms fail during eligibility, before lowering/VM validation. Document this deliberate producer boundary.
- Validation: cargo fmt; cargo clippy -- -D warnings; cargo test; yarn compile and yarn check-agent-interface via yarn check-all. Agent interface: 18/18; observed expression-type query 239.01 ms / 2,957 stdout bytes, source-backed data type 232.94 ms / 938 bytes (local debug smoke, not performance evidence).
- No benchmark report or profiler asset is added. This does not complete the real application-owned consumer work in #887.
