# 记录 Calcit-first 测试放置规则

## 背景

近期编译器改动更依赖 Rust 单元测试，而 definition `:tests` 增长较少，review 时需要先理解 Rust 内部结构，也削弱了从 Calcit 程序视角验证语言行为的机会（calcit#989）。

## 修改

- 在 `AGENTS.md` 新增「测试放置（Calcit-first）」小节：用户可观察语义默认写 definition `:tests`；parser/serializer、内部结构、host/FFI 边界、内存/并发 invariant 与错误恢复留在 Rust；backend-specific 实现同时要有 Calcit 侧共享语义测试；不新增比例或 coverage 指标。
- 在 `docs/features/testing.md` 新增 `Choose the Test Surface (Calcit-first)` 与 PR checklist，并以 Tag 切片（`calcit.core/&compare` 的 `:tests` + Calx Rust fixture）作为示例。
- CI 与 `yarn try-core-tests` 的 definition-attached core test 命令加 `--require-match`，避免 tag/scope 选择错误时出现“零测试通过”。

## 验证

- `cargo run --bin calcit -- src/cirru/calcit-core.cirru --compat-types test --tag unit --summary-only --require-match`（241 通过）
- 文档改动，无需 Rust 构建验证；CI 继续执行 definition-attached tests。
