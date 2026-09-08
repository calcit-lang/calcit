# Complete definition metadata / 完整定义元数据

- Fix #875: add `query def --format json` schema-version 1 envelope, structured EDN FFI and lossless `ffi_edn`; keep legacy `--json` mixed output and its string field compatible. Raw human output no longer truncates FFI; bounded human output is explicitly a preview.
- Regression coverage: 300 member mappings, escaping/Unicode/newlines, absent FFI, source-less builtins, legacy/raw parity, parse failures and single-JSON stdout. Interface IR v2 is unchanged.
- Consumer candidate: js-ffi's 149-definition catalog uses the new query; persisted schemaData still uses Snapshot decoding. Node tests and 71 browser assertions pass with this unreleased candidate CLI (no formal version pin before publication).
- Validation: cargo fmt; cargo test (766 library / 312 CLI plus WASM tests); yarn check-all; 20 agent protocol scenarios plus full definition round trips. Complete local definition: 13.14 ms / 1385 bytes; builtin: 62.71 ms / 580 bytes. DomElementHost debug candidate warm 28.47–29.43 ms / 2597 bytes vs installed 0.14.2 release 11.22–11.50 ms / 3282 bytes (different build profiles, not a speed comparison).
- Release-gate hygiene: #918 adds the missing shared-policy lock to an existing field-access test; Rust 1.98.1 Clippy's three constant chunks_exact warnings are mechanically updated to as_chunks without changing remainder behavior.
- 兼容旧输出，机器输出明确区分结构化数据与完整 EDN；不扩展 async/public checker 范围。真实消费者验证后再发布，后续正式版本 pin 和 release 由 #909/#914 跟踪。
