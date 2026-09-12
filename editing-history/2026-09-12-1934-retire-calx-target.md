# 退役 Calx target，收敛到 WASM backend

## 决策

Calcit 不再生成或维护 Calx target。已完成的 Calx 实验保留在 Git 历史中，但不再占用主仓库的依赖、类型、lowering、测试和文档维护面。后续 backend 投入集中到现有 WebAssembly lowering，并继续以统一 typed core 语义约束 native、JS 与 WASM。

## 清理范围

- 删除 `calx_vm` 直接依赖，以及 lockfile 中只由它引入的传递依赖。
- 删除 `src/codegen/calx.rs` 和 Calx eligibility、coverage、lowering、program contract、cache、benchmark session adapter。
- 删除 Calx 专属 Rust integration tests、source/golden fixtures 与运行文档。
- 删除只服务 Calx kernel ABI 的 `F64Buffer` runtime/type variant、三个内部 primitive 和 core Snapshot definitions。
- 更新 AGENTS、分层语义 RFC、测试指导、类型指导和文档索引，明确 WASM 是优先编译 backend。

## 保留边界

- 不删除历史 editing records，以保留此前实验的设计和验证证据。
- 不改变现有 WASM emitter、runtime、scripts 或执行用例；这次先缩小维护面，再按 #1011 的独立语义切片扩大 WASM 覆盖。
- compiler test 使用的 isolated source namespace helper 仍被 preprocess 测试复用，因此保留并限制为 `cfg(test)`，不作为外部 adapter surface。

## 验证

- `cargo test --all-targets --quiet`
- `cargo clippy --all-targets -- -D warnings`
- `corepack yarn check-all`
- 251 个 Calcit core definition tests
- Native、JS、IR 与完整 WASM execution suite
