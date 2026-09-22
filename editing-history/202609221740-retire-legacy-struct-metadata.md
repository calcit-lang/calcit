# 退役旧 JS Struct metadata 布局回退

- `CalcitStructDef` 构造时已经按字段名规范化字段和类型顺序；当前 runtime 与 codegen 传给 `CalcitStructValue` 的定义都来自此构造器。旧值构造器再次检查并重建非规范化 `structRef` 的分支只服务于手工传入旧式普通对象。
- 移除该回退，保留 Struct 值输入字段和值的规范化，以支持合法的逆序输入和 Cirru EDN 解析；正式定义对象保持引用身份。
- JS 边界测试改为使用真实 `CalcitStructDef`，覆盖逆序字段/值对齐及定义引用。没有改变 Calcit 表层语义，因此不增加重复的 `:tests`。
- 普通 trait 的 tag method key 在 editor 与 gen-code-strict 仍有真实使用，应先完成生态迁移；external-object 的 `:field` 不是旧语法，不应退役。
