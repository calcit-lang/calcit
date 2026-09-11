# 收敛迁移分析器

## 背景

严格预处理已经负责判断 Dynamic 是否能进入 typed code，但 `weak-types` 仍额外递归计算 Dynamic 分子、类型节点分母、比例、intent 对账，以及 schema shape/family 的多层排名。这些统计不参与编译语义，却要求独立扫描和持续维护策略说明。

## 消费者盘点

- calcit-lang 组织内多个仓库仍在 CI 使用 `analyze quality --baseline`，包括 calcit-http、calcit-viewer、corokia、calcit-fetch、calcit-graphviz、calcit-wasm-play、phoenix-script、reacher、recollect、respo-calcit-workflow 等，因此 0.14.x 继续读取现有 v1/v2 baseline。
- 多个 workflow 使用 `check-types --summary-only` 与 `weak-types --intent unresolved` 定位迁移项；calcit-lang.org 还读取 weak-types 的 kind/intent 汇总。
- 组织代码搜索没有发现 `data.summary.dynamic_usage`、schema shape/family 聚合或 occurrence `impact` 字段的外部消费者；仓库内只有协议自测与文档引用。

## 修改

- 删除 Dynamic 比例、总位置数、intent 对账和对应的额外两次 scope 扫描。
- 删除 human report 的 schema shape/family 聚合及其解析 helper。
- 删除 occurrence `impact` 策略；保留 kind、intent、definition、path、detail、evidence 与 suggestion 作为迁移定位事实。
- 保留 `check-types`、`weak-types`、`quality` 命令和 baseline 读取，避免打断现有 0.14.x CI。
- 文档明确默认严格 warning/error 是类型语义来源；新项目不生成 baseline，存量 baseline 只在 0.14.x 递减使用，清零后移除，0.15 再按 release migration note 删除剩余数量策略。

## 验证

- `cargo test --lib -- --test-threads=1`
- `cargo test --bin calcit -- --test-threads=1`
- `cargo clippy --all-targets -- -D warnings`
- `yarn check-agent-interface`
- `yarn check-core-quality`
- `yarn check-core-dynamic-classification`
- `yarn check-all`
- `cargo run --bin calcit -- docs check-md docs/type-guidance.md --entry calcit/test.cirru --failures-only`
- `cargo run --bin calcit -- docs check-md docs/run/library-quality.md --entry calcit/test.cirru --failures-only`
- 使用当前分支构建的 `calcit` 在本地 `calcit-lang/calcit-http` 执行其已提交的 v2 `config/calcit-quality.cirru`，九项 delta 均为 0，baseline 兼容通过。

实现与回归测试合计净减少超过 300 行，weak-types JSON 从三次 scope 扫描降为一次。
