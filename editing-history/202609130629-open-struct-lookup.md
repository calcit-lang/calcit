# 让开放值通过普通 `get` 查询运行时 Struct 字段

时间：2026-09-13 06:29 +0800

## 背景

#993 中的生态适配器接收显式 `Dynamic`，需要按运行时 tag 同时查询 Map 与 Struct。静态已知 Struct
必须继续使用 `(:field value)`，但开放值此前只能退回被 strict mode 禁止的 `&struct:get`。

## 调整

- 普通 `get` 在运行值为 Struct 时按 tag/string/symbol 查询字段，命中返回 `Option<Dynamic>`，缺失或无效字段键返回 `%none`。
- 静态已知 Struct 的预处理拒绝保持不变，并更新诊断，明确开放查询只适用于显式 `Dynamic` receiver。
- 不新增 `get-dynamic`，不放宽 raw primitive 门禁，也不让开放查询替代具名 Struct 的必填字段证明。
- 为 `calcit.core/get` 增加带 `:core :unit` 标签的 definition test 和示例；在 Calcit Struct 测试入口通过一个声明为 `Dynamic` 的函数参数覆盖 native/生成 JS 共同行为。
- 同步 Struct、升级、quick reference、Agent 与多态文档中的两类访问契约。
- 将 Agent interface 的 source-filter 搜索检查从固定命中数改为验证 summary 自洽、结果非空与 origin/source 正确，避免 core 测试内容变化触发无语义关系的统计失败。
- Dynamic 分类清单不再嵌入整个 Snapshot revision；清单仍由实际 schema-Dynamic 行逐项生成和比较，但普通代码、文档、示例或测试变化不会要求刷新无关统计文档。

## 验证

- `calcit.core/get` 的 4 个 definition tests 与 5 个 examples 通过。
- 默认 strict `read-field(Dynamic, Tag)` 复现返回 `[(%some |demo) (%none)]`。
- 默认 strict 下直接对已知 `FsPath` 调用 `get` 仍报告 `E_UNSUPPORTED_INDEXED_RECEIVER`。
- `yarn try-rs` 与 `yarn try-js` 通过，共用 Calcit 侧开放 Struct 查询断言。
- `cargo test` 全量通过（772 个 lib、340 个 CLI、17 个 cr-wasm、4 个 fix CLI 测试）。
- `yarn check-all` 全量通过，包括 258 个 Calcit definition tests、32 个 Agent interface 场景与 WASM 集成检查。
- `docs/CalcitAgent.md` Markdown 检查通过；另外两份既有文档的单独检查仍命中与本次段落无关的历史 strict 示例失败，全量仓库门禁不受影响并已通过。
