# Static analysis and Agent CLI integration

## 静态类型

- 将 `:any` 定义为明确的静态顶类型，与会双向擦除检查的 `:dynamic` 区分。
- 增强 list/map/set/ref、record/enum、callback、rest 参数与局部函数的类型推断和 schema 检查。
- 让 type coverage/weak-types 输出稳定 JSON、详细路径、建议与 `--summary-only` 汇总。

## Agent 语义查询

- 增强 `cr query schema/type/type-at/context/search/search-expr`，提供带 revision 的机器可读结果。
- 缩小查询时的 Snapshot/module 加载范围，并为定义 examples 增加精确检查入口。
- 增加 `yarn check-agent-interface`，验证 JSON stdout 协议并记录查询耗时与输出体积。

## CLI 编辑完整性

- Cirru AST 输入严格使用 `quote` 作为代码/数据边界，不再把普通 EDN 误当代码。
- `edit schema` 要求单个 quoted node；批量 `edit examples` 要求每个顶层节点独立 `quote`，从而同时可表示 leaf 和表达式。
- `query examples` 直接显示 leaf；rename/move 定义时同步更新声明名称。
- 清理文档与提示中的历史短参数，统一为明确的长参数词汇。

## 验证

- `cargo fmt --all`
- `cargo test`
- `cargo clippy -- -D warnings`
- `yarn compile`
- `yarn check-all`
- `cr docs check-md docs/CalcitAgent.md --entry calcit/test.cirru --failures-only`
- 通过 Snapshot 临时副本验证 schema/examples 的 quoted expression、quoted leaf 与裸输入拒绝路径。
