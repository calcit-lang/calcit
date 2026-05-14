# 2026-03-13 22:16 validate struct type arg arity

## 背景

发现类型注解里可以写出 `:: Pair :number :string` 这样的形式，即使 `Pair` 是非泛型 `defstruct`。旧逻辑会把后续类型参数直接附着到 `Struct(...)` 注解上，但并不会校验 `generics` 个数，最终在值匹配时只看 struct 名字，导致漏检。

## 本次修改

- 在 `src/calcit/type_annotation.rs` 为 `CalcitTypeAnnotation` / `CalcitFnTypeAnnotation` 增加 `validate_applied_type_args()`。
- 对 `Struct` 注解增加规则：
  - 非泛型 struct 不允许携带类型参数；
  - 泛型 struct 必须严格匹配声明的类型参数个数。
- 对 `Enum` 注解增加保守规则：当前不接受额外类型参数。
- 在 `src/builtins/records.rs` 中，`defstruct` 字段类型解析后立即做校验。
- 在 `src/calcit/sum_type.rs` 中，`defenum` payload 类型解析后立即做校验。
- 在 `src/calcit/sum_type.rs` 与 `src/calcit/type_annotation.rs` 补充回归测试，覆盖：
  - 非泛型 struct 带类型参数时报错；
  - 泛型 struct 类型参数个数不匹配时报错；
  - enum payload 中引用非法 struct 类型注解时报错。
- 同时修正 `calcit/test-generics.cirru` 中 `Wrapped` 的 payload 类型，把错误的 `:: Pair :number :string` 改为 `Pair`。

## 结论

现在像 `:: Pair :number :string` 这种写法不会再静默通过；定义期就会被校验出来，避免文档和测试继续误导后续修改。
