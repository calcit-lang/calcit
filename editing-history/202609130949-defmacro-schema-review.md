# 补强 defmacro schema review 覆盖

## Review 结论

只断言 `:required`、`:optional`、`:rest` 等字段出现，不能阻止计数错误、意外 rest、错误 payload
或非空 capabilities 回归。CLI 集成测试改为对 required、optional、rest 三种参数形状分别比较完整的
结构化 schema AST，包括 `Expr<Dynamic>` 与空 capabilities。

原来的失败用例只覆盖“参数节点不是 list”，没有进入共享 `ParamShape::from_tokens`。新增以下顺序错误：

- rest binding 后又出现普通 binding；
- rest marker 后直接出现 optional marker；
- rest binding 后又出现 optional marker与 binding。

每次失败后都重新比较原始 Snapshot 字节，并在全部失败后执行严格 `--check-only`，证明任何分支都没有
留下部分写入或不可加载文件。
