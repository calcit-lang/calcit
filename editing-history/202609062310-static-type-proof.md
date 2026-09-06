# Separate compatibility from static proof / 分离兼容与静态证明

Issue: #842. Milestone: 0.14.0 — Strict by default.

The annotation relation now exposes compatibility and static proof as distinct
operations. Compatibility retains the legacy acceptance rules used by runtime
validation and diagnostics, while proof reports `Proven`, `NeedsBoundary`, or
`Mismatch` and commits generic bindings transactionally. Dynamic values,
legacy `any`, unknown callables, callable tags, unresolved type slots, legacy
nullish widening, recursive type variables, and erased applied arguments no
longer count as concrete proof.

Generic return inference, `apply`, method argument projection, enum/struct
payload inference, strict generic binding, homogeneous collection inference,
callback inference, and Ref proof now select the appropriate relation
explicitly. Projection inference may retain concrete bindings that are
independent of an open boundary, such as `Result<Number, Dynamic> -> Number`,
but a mismatch rolls back the whole candidate and Dynamic itself never binds a
concrete type variable. Compatibility-based branch joins retain the weaker
type instead of sharpening Dynamic evidence.

类型注解关系现在明确区分旧兼容判断与可授权静态推断的证明。兼容路径继续服务运行时
校验和诊断；证明路径返回 `Proven`、`NeedsBoundary` 或 `Mismatch`，并以事务方式提交
泛型绑定。Dynamic、旧 `any`、未知函数、Tag 函数、未绑定 type slot、旧 nullish
扩宽、循环类型变量及擦除的类型参数都不再被当作具体证明。

泛型返回值、`apply`、方法参数投影、enum/struct payload、strict generic binding、
同质集合、callback 与 Ref 不变量均显式选择关系。与开放边界无关的具体投影仍可保留，
例如 `Result<Number, Dynamic> -> Number`；但 mismatch 会整体回滚，Dynamic 本身不会
绑定成具体类型。基于兼容性的分支合并始终保留较弱类型，避免把开放证据反向收紧。
