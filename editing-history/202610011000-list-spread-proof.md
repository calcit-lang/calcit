# 展开参数的已知集合类型证明

## 依据与范围

issue #1583 来自 Memkits/manuscript 的真实编辑路径：`vals` / `filter`
保留 Set 类型，但 `dissoc changed-drafts & empty-ids` 通过预处理，直到
native 的 List 展开或 JS 的 `listToArray` 才失败。当前 main 同样可以复现。

这次只补足 runtime 展开要求的集合种类证明。复用既有
`resolve_type_value` 和方向性的 `prove_with_bindings`，对 `List<Dynamic>`
的明确 Mismatch 报 `E_SPREAD_TYPE_MISMATCH`。共享检查覆盖命名函数、
primitive / local callable，以及显式 `&call-spread`；每个 `&` 只作用于
紧随其后的一个参数，不影响其他普通参数。

Dynamic、未解析类型和 macro syntax 的 NeedsBoundary 不冒充已证明的
List，也不在这个修复中强制迁移。参数数量、List 元素到 callable 参数的
证明仍由 #1538 继续收敛。没有新增开关、检测命令、白名单或类型关系表。

## 迁移与兼容边界

合法 List 展开不变；Set 应通过实际公开方法 `.to-list` 显式转换。
Set 顺序没有保证，因此提示不是自动 fix，不替用户决定参数顺序。
空 List 本身合法，但目标函数仍须满足其运行时 arity；测试保留 `dissoc`
至少一个 key 的既有契约，不用本次修复顺便修改 arity。

## 验证

公开正向语义位于 `calcit.core/dissoc :tests`，通过 Calcit CLI 写入：
List key、Set 显式转换、展开后普通参数、多处展开、空展开、局部 callable
及异构 List 展开。
原有两项 dissoc 测试不变。脚本只搬运这些共享测试到真实 JS 产物，并检查
不能进入 test execution 的错误程序的 CLI envelope。

12 个错误程序覆盖 Set 字面量 / 局部 / `vals` / `filter`，scalar、Map、
Option、primitive、local callable、显式低层展开、多处展开。每项分别验证
native、check-only（含 compatibility 模式）、JS、WASM 都在预处理阶段报告同一代码、期望/实际类型、
位置及转换提示；没有声称 WASM 已经支持合法 variadic List 展开。
