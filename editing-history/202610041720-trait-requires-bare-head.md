# trait 的 requires 子句头使用裸符号

`deftrait` / `defexternal` 的父 trait 子句原写作 `('requires Parent)`，头部是引用符号。与 `deftrait` 其余写法相比多一个引号，也不符合“声明性子句头直接写关键词”的一致性，因此首选写法改为 `(requires Parent)`。

## 兼容边界

- `deftrait` 宏、预处理识别和 `defexternal` 规范化同时接受 `requires` 与旧的 `'requires`；二者展开结果相同，Snapshot 中已有的 `'requires` 不需要迁移。
- 成员键仍然只能是 `:field` 或 `.method`，裸符号 `requires` 不会与它们混淆。
- 诊断文案与 JS 运行时错误去掉引号，错误码 `E_TRAIT_REQUIRES` 不变。
- 不新增 `calcit fix` 规则，也不标记 deprecated；下游 js-ffi 与 Respo 可在方便时自行改写。

## 验证方式

`calcit.core/deftrait` 的 definition `:tests` 比较两种写法的宏展开；Rust 测试覆盖 `defexternal` 对两种写法的保留；`test-traits`、`js-ffi-module` 与 type-fail 夹具改用新写法并保持原有诊断。
