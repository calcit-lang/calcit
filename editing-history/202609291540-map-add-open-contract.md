# Map `.add` 的旧 entry 契约改为开放边界

## 背景

旧 `&map:add-entry` 接受一个二元 List，运行时将第 0 项当键、第 1 项当值。现有 `List<T>` 不能分别证明这两项符合接收者的 `Map<K,V>` 类型；此前 schema 却返回 `Map<K,V>`，使 `query type` 把 `.add` 误标为已证明。

## 调整

- 保留旧 `.add` 的运行时实现和二元 entry 检查，不改变已有程序的运行结果。
- 将其结果契约收窄为只能证明 Map 形状，即 `Map<Dynamic,Dynamic>`；CLI 查询状态为 `open`。
- 新代码首选参数独立、类型可证明的 `.assoc key value`。不自动改写旧 `.add`，因为调用形状与失败行为不同。
- 在定义附带的 `:tests`、生成 JS 与 WASM 中覆盖兼容行为；CLI 测试确认 `.add` 开放且 `.assoc` 保持已证明。

## 限制

`Dynamic` 边界仍可由显式类型断言进一步收窄，因此本次修复的是错误的自动证明及 Agent 查询指引，并不宣称旧入口获得严格的 K/V 类型安全。后续退场策略由 #1458 和 #1479 跟踪。
