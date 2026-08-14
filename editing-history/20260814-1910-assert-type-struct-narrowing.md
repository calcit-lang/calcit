# 2026-08-14 assert-type Struct 收窄

- `assert-type` 的限定名（例如 `'app.schema/Store`）在非泛型上下文中必须保留为名义 `TypeRef`，不能误作 type variable；这样后续必填 Struct 字段访问才能解析声明。
- 断言包裹任意表达式时，类型推断应把声明类型交给外层 `let` binding；局部变量的断言仍由预处理阶段直接收窄。
- `defstruct` / `defenum` 的 `$ {} ...` map-headed 输入已经由宏归一化，静态类型定义解析也必须在泛型和 `:where` 解析前采用相同的归一化。
- 回归覆盖包括两种断言收窄路径、map-headed Struct/Enum 静态解析，以及完整 Struct/Enum Snapshot 运行。
