# Core CI 不再使用 Dynamic 数量预算

## 背景

内置 core 的 `analyze quality --baseline` 曾在本地全量检查、PR CI 与发布流程中充当数量门槛。该检查按 definition 比较 Dynamic/type-coverage 命中数，不能代表类型正确性；继续维护数量预算会让新类型推导与语言语义的设计受到统计规则牵制。

## 调整

- 从 `check-all`、PR 与发布流程中移除 core quality baseline 调用，并移除 `check-core-quality` 快捷入口。
- 保留显式 `analyze quality --baseline` 的旧项目兼容能力；`config/calcit-core-quality.cirru` 暂留作历史参考，不再因 CI 更新。
- 保留 core Dynamic 逐位置清单及同步检查，作为人工审阅开放边界的文档校验，而不是类型正确性或数量预算门槛。
- 更新生成器和文档中误称 baseline 仍锁定新增位置的说明。

## 验证与后续

`yarn check-all` 通过（含 304 个 core definition `:tests`）；Dynamic 文档同步检查、`cargo test -q`、`cargo clippy -- -D warnings`、`cargo fmt --check` 与 Markdown 文档校验（340 个片段）通过。额外尝试的 `cargo clippy --all-targets -- -D warnings` 在未改动的 `src/codegen/emit_wasm/edn_parse.rs` 上遇到已有 `items_after_test_module` 告警；该文件不属于本次范围。core Snapshot 的默认入口不是可直接用于 `--check-only` 的完整公开定义检查，因此尚需补齐公开定义的严格可达性验证；不要以新的数量规则填补这一缺口。#1238 保持开放。
