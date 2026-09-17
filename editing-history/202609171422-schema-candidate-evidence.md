# Schema 与结构候选证据

## 背景

`synthesize-schema-v1` 已经能够复用编译器和 resolver call-site 证据补全单个 definition 的 schema，
但项目级审阅只能逐个猜测目标。另行增加推断命令会扩大 CLI surface，并产生两套不一致规则。

## 修改

- 在 `analyze weak-types` 增加显式 `--schema-evidence`，复用原有 schema synthesis 推断；普通
  `weak-types` 仍只读取 Snapshot。
- 候选区分 `exact`、`usage-derived`、`boundary-unknown` 与 `conflict`，保留 unresolved slot、
  resolver 确认的调用点和稳定信息代码；所有候选均为 `review-required`。
- 保守聚合重复匿名 Map 的字段形状，并从 `match` 分支提取 tag dispatch 候选。两类候选只包含
  建议名称、源码路径和冲突信息，不创建 Struct/Enum，也不进入自动 fix。
- `fix --workflow strict` 复用相同证据，避免 Agent 再调用平行 inventory 工具。
- `analyze.weak-types` envelope 升级到 v8；Cirru EDN 仍是首选结构化输出，JSON 用于互操作。

## 边界

- schema evidence 只在显式开启时以兼容诊断模式预处理项目 definition，不执行 entry 或 host effect。
- 无法预处理的 call-site owner 会显式进入 evidence 并把置信度降为 `boundary-unknown`，不阻断 inventory，
  也不允许残缺证据冒充 exact。
- `--deps` 可以为证据提供依赖信息，但 schema 写回候选只针对当前项目 definition。
- 自动写回能力没有扩大：仍需显式选择 `synthesize-schema-v1`，且只有无剩余洞的候选可应用。
- 工具不决定 Option/Result、业务默认值、FFI trust、Struct/Enum 命名或领域边界。

## 验证

- Rust CLI 集成测试覆盖 exact、usage-derived、conflict、重复 Map shape、dispatch、summary-only、
  Cirru/JSON envelope 与 strict workflow 复用。这些测试验证 host CLI 和序列化边界，因此保留在 Rust；
  没有新增 Calcit 运行语义。
