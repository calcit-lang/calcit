# Map 去重值的命名

## 决策

现有 `vals` / Map `.values` 先经 `&map:vals` 得到值列表，再转成 Set；重复值会消失，返回顺序也没有保证。新增 `distinct-values` / `.distinct-values`，明确返回 `Set<V>` 的语义。保留旧前缀与方法入口，避免把既有 `.values` 误读成保留重复值的视图；真正的保重复值 API 留待独立设计。

## 改写边界

显式 `core-map-distinct-values-v1` 只处理完整的零参数 `.values` 方法调用。具体 Map 接收者、新旧方法的 proven 契约、同一 core 实现及稳定的源码上下文全部成立时，才提供自动改写。开放类型、未知 macro、quoted data、用户方法、前缀 `vals` 和 definition 附带的 tests/examples 不自动改写。沿用现有 fix 的 revision 与 staged validation，不加入已发布的 preset。

## 验证

Calcit 定义级 `:tests` 覆盖重复值、空 Map、新方法与旧别名，4/4 通过。CLI 集成测试覆盖预览、应用、过期 revision、幂等、quoted/macro 边界；未标注类型的接收者被严格预处理拒绝，不会获得自动改写。静态 Agent 查询确认新旧方法均 proven，返回 `Set<Number>` 且同指一个 core 实现。完整 `cargo test`、Clippy、`yarn compile`、`yarn check-all` 和 Markdown 检查通过；原生执行、生成的 JS 与实际实例化的 WASM 都得到去重计数 2。Respo 严格检查 172/172 定义通过、0 diagnostics。
