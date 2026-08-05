# Explicit Unit without nil forms

## 修改概要

- 将十个测试 fixture 的空 `reload!` 改为空函数体，并把 schema 从 `Dynamic` 收紧为 `Fn() -> Unit`；
- 将五个 dummy trait callback 的 `;nil` body 改为空 body，同时把对应 trait method 从无签名 `:fn` 收紧为泛型参数到 `Unit` 返回；
- 删除 `&init-builtin-impls!` 已由 `Unit` schema 覆盖的显式 `;nil` else，并让 `;nil` 宏自身用空 body 自然产生 Unit；
- 为 `;nil` 增加一个只验证兼容边界的 Unit example；
- `analyze weak-types --only code-nil` 现在同时识别裸 `nil` 与 `;nil`，并提示已声明 Unit 的代码删除多余显式 nil form；
- 测试代码移除 15 个 `;nil`，core 普通实现再移除 2 个显式 nil form；只在专门验证 `;nil` 兼容语义的 example 中保留一次调用。

## 知识点

- Calcit 空 `defn`/`fn` body 会自然返回运行时 Unit；有显式 `Unit` schema 时，不需要用 `nil` 或 `;nil` 重复表达“无返回值”；
- trait method 可以用 `:: :fn`、泛型 receiver 与 `'Unit` 返回声明 no-value contract，避免 `:fn` 抹掉 callback 返回类型；
- `;nil` 仍可作为旧宏或必须提供 AST 占位节点的兼容工具，但不能成为绕过 nil 审计的方式；
- 顶层 eval 的 `(;nil)` 会形成额外一层调用并二次求值；验证宏本身应使用顶层 `;nil`，嵌套参数位置才写 `(;nil)`。

## 验证

- focused weak-types Unit/`;nil` tests
- modified fixture check-only/runtime tests
- `calcit.core/;nil` example
- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`
- Respo type coverage 与 nil debt 回归
