# 内置初始化函数实际返回 Unit

`calcit.core/&init-builtin-impls!` 的既有 schema 是 `Fn() -> Unit`，但原函数以
没有 else 的 JS 注册条件表达式结尾。实际 native 求值得到 nil；同一返回值断言在
JS 已通过。这使内部运行时行为与声明合同不一致，也触发完整 strict 的返回证明错误。

仅在原函数最后追加 `&unit`。原 impl 加载、identity 表达式和 JS 注册分支逐项保持原样，
不改 schema，不吞注册异常，不新增检查规则、API、Dynamic 或 unsafe。Snapshot 通过
官方事务 dry-run、scoped revision 守卫修改，并完整 query readback 确认只追加这一叶子。

在已有的真实初始化调用者 `test-traits.main/main!` 添加一项 definition `:tests`：
对连续两次初始化的返回值断言实际等于 `&unit`、实际类型为 `:unit`，随后验证公开 Enum
empty 查询及集合 count 行为。保留 fixture 原代码、schema 和全部原测试。测试沿用原
native/JS 回放，不把 WASM 运行时 impl 构造列为本次新增能力，不新增排除或运行器。

修复前，同一有效测试在 native 因实际 nil 而失败、在 JS 通过；修复后两者都通过。
这里的成功是实际返回值回放，不用函数声明替代运行时结果。原完整回归及精确提交 CI
的执行终态放在关联 PR 中。

重建完整源码候选后，限定 concrete-return audit 已 exit 0、diagnostics 为空。
原 trait 完整 strict 的类型诊断也为空，可达项目定义检查通过；整个 workflow 仍 exit 1，
因为原有迁移建议与待审边界尚未收尾，不能写成整个 strict workflow 已完成。
本次没有应用那些迁移或放宽其他类型。开放 Struct 的 WASM/WASI 分发及开放 Enum 的
实际 impl 身份分别保留在原问题中。
