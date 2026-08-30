# Multi-target FFI bindgen preview / 多目标 FFI bindgen 预览

- Add a deterministic Interface IR v1 preview consumer that emits Rust adapter
  stubs, Calcit raw wrappers, TypeScript declarations, WIT, and a SHA-256
  manifest from the same document.
- Gate the preview to supported native sync edn-buffer-v1 bindings and fail on
  unsupported definitions or unresolved WIT named types; never generate a
  Dynamic fallback.
- Add calcit.std 0.2.29 MD5 golden fixtures, repeatability tests, and the check
  to `yarn check-all`.
- Document the preview as a Phase 0 measurement tool that will move to an
  independent bindgen crate after lifecycle fields stabilize.

- 增加确定性的 Interface IR v1 preview consumer，从同一文档生成 Rust adapter
  stub、Calcit raw wrapper、TypeScript declaration、WIT 与 SHA-256 manifest。
- 仅接受 supported native sync edn-buffer-v1 binding；unsupported definition
  或未解析 WIT named type 会直接失败，绝不生成 Dynamic fallback。
- 增加 calcit.std 0.2.29 MD5 golden fixture、重复性测试，并接入
  `yarn check-all`。
- 将该工具记录为 Phase 0 量化手段；生命周期字段稳定后再迁移到独立 bindgen
  crate。
