# 继续清理 Struct 命名

延续 data model v2 的术语迁移，继续移除当前 Calcit 代码路径中把 `Struct` 称作 `record` 的内部绑定、诊断和测试表述。

## 本次改动

- EDN 解码、详细快照、构建脚本和运行时快照读取中的 `Edn::Struct` 局部绑定统一为 `struct_value`。
- 当前 Struct/Enum 诊断、WASM/JS codegen 文案和 CLI 查询提示统一使用 `struct` / `enum`。
- `CalcitEnumDef` 从 Struct 原型收集 variant 的内部参数、Struct map 转换和编辑器 EDN 修改辅助函数同步改名。
- Struct 文档别名和类型注释同步更新。
- 保留旧 API 名称、旧 schema 关键字、`Record(...)` 调试输入以及 `test-wasm.cirru` 等明确的兼容测试内容。

## 验证

- `cargo fmt --check`
- `cargo check --all-targets`
- `cargo clippy --lib -- -D warnings`
- `cargo test data::edn::tests`（5 项）
- `cargo test data::edn_decode::tests`（7 项）

`cargo test snapshot::tests` 中已有的 `test_save_snapshot_round_trip_keeps_real_world_schema_markers` 仍失败于保存内容断言；本次仅修改绑定名和诊断文本，未改变快照格式逻辑。
