# 完成 bindgen preview cutover / Complete bindgen preview cutover

## 中文

- `calcit-bindgen#10` 已合并，独立仓库具备 deterministic Rust/Calcit/TypeScript/WIT generation、backend-scoped manifest/check、compatibility diff 与 WIT validation。
- 使用全局 Calcit 0.13.72 从最新 `calcit.std` 0.2.29 现场导出 Interface IR v2；standalone validate/generate/check 与 `wasm-tools component wit` 通过。
- 真实导出的四类 backend 产物与已执行 generated dylib request/response/free smoke 的 md5 contract 逐字节一致。
- 从 core 删除 JavaScript preview generator、preview tests、goldens/fixtures 和 package script；保留 exporter、v1/v2 schema、Rust conformance tests 与权威 Interface IR 文档。
- 文档改为指向独立 production generator，并在 README/AGENTS 固化 core 与 standalone 的职责边界。

## English

- `calcit-bindgen#10` is merged with deterministic Rust/Calcit/TypeScript/WIT generation, backend-scoped manifests/checks, compatibility diffing, and WIT validation.
- Calcit 0.13.72 exported a fresh Interface IR v2 document from current `calcit.std` 0.2.29; standalone validate/generate/check and `wasm-tools component wit` passed.
- All four outputs from the live export are byte-identical to the md5 contract covered by the generated dylib request/response/free smoke.
- Removed the JavaScript preview generator, preview tests, goldens/fixtures, and package script from core while retaining the exporter, v1/v2 schemas, Rust conformance tests, and authoritative Interface IR documentation.
- Updated README/AGENTS/docs to make the core-versus-standalone ownership boundary discoverable.
