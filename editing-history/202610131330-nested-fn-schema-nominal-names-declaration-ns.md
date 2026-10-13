# 嵌套 Fn schema 的 nominal 名称按声明命名空间解析

#2027：被调函数的 Fn schema 中，嵌套回调参数写成短名（如 `'State`）。内联回调在调用方命名空间
预处理时，上下文推断把该短名按调用方命名空间解析，得到未解析的 `'State`；严格证明比较的却是
按声明命名空间解析后的 `quatrefoil.types/State`，于是出现 `E_CALL_ARGUMENT_UNPROVEN`，
回调里读取 `(:x state)` 也找不到声明字段。

修复在调用点生成期望参数类型时，先用被调函数的声明命名空间限定 schema 中的 nominal 引用，
再交给回调上下文。复用现有 `resolve_namespace_type_refs_for_body`，不新增诊断、fix 规则、
转换或 Dynamic 放宽；无法解析的名称保持原样，严格门禁不变。

回归使用 `tests/fixtures/nested-nominal-ns.cirru`（两个命名空间，含读取字段的 `:tests`），
在 `tests/fix_cli.rs` 中验证严格 workflow，并以完全限定名写法作为对照。该场景依赖多命名空间
Snapshot 与 `calcit fix` 严格流程，放在现有 CLI 测试 harness。
