# 严格检查按 definition 继续

## 问题

普通 `--check-only` 遇到首个阻断诊断即退出，升级项目时 Agent 需要反复运行才能发现互不依赖的问题；如果直接在失败表达式后继续推断，又会把缺失依赖证据误报成新的类型错误。

## 调整

- 在既有 `--check-only` 入口增加 `--keep-going`，先从 init/reload 的静态可达图生成稳定的依赖优先顺序。
- 只跨 definition 恢复：当前 definition 的 warning/error 标记为 `failed`，依赖失败的调用者标记为 `blocked`，无法安全归属的异常标记为 `cascaded`。
- human 报告使用 Markdown 边界；Cirru EDN 与 JSON 输出共享单一 versioned envelope，并保留非零失败退出码。
- 默认 fail-fast 行为不变；zero-debt quality gate 保持为修复严格预处理问题后的独立检查，避免把统计预算混入诊断聚合。
- CLI 回归覆盖多个独立错误、依赖阻断、稳定排序、human/EDN/JSON 和参数约束；`yarn check-agent-interface` 固定机器协议。

## 验证

`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test --all-targets --all-features`、`yarn compile`、`yarn check-agent-interface`、`yarn check-all`。
