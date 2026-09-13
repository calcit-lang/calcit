# 声明式验证 Profile 与共享项目加载

本次为 milestone 0.14.18 的 #1073 增加 `calcit analyze verify --profile <name>`，把重复的 Calcit-owned 静态门禁收敛到既有 `analyze` 入口，而不是增加新的顶层命令。

## 设计要点

- Snapshot 顶层 `:verification` 使用显式 `:schema-version 1`，profile 按声明顺序选择 named entries、checks 与 `:on-failure` 策略。
- v1 只组合只读静态能力：严格 init/reload 预处理、dynamic-method finding 的 zero-debt 门禁，以及既有 quality zero-debt 分析。测试、文档执行、外部命令和生成产物继续显式运行，避免 profile 变成通用任务执行器。
- Snapshot 和内置 core 只解析一次；相同 module path 在本次命令内缓存。同一 entry 的 strict 与 dynamic-methods 共享一轮预处理和类型推导结果。
- Human 输出使用 Markdown heading/list；JSON stdout 保持单一、确定的 envelope。每个结果带 profile、check、entry、target、revision、status 与稳定 diagnostic code。
- 配置在检查前 fail closed：未知 schema version、字段、entry、check、重复项和空列表均拒绝。`config show` 同时暴露 profile，Snapshot formatter 与 program diff 保留其结构。

## 验证

- Native 与 JS-target fixtures 使用同一 profile contract。
- 覆盖 JSON/human 输出、配置失败不运行检查、`:stop` 第一项失败即停止、Snapshot round-trip。
- 运行 `cargo fmt --all`、`cargo clippy --all-targets -- -D warnings`、`cargo test`、`yarn check-agent-interface`、`yarn check-all` 与新增文档的 `docs check-md`。
