# 泛型函数 TypeVar 返回类型推导 (identity 用例)

## 概要

实现 `hint-fn (generics 'T)` 语法用于函数泛型声明，使 `identity` 等泛型函数的返回类型能在调用处根据实参类型自动推导。

## 核心改动

### `src/calcit/type_annotation.rs`

- 新增 `substitute_type_vars(&self, bindings) -> Arc<CalcitTypeAnnotation>`：将注解中的 `TypeVar` 替换为 `bindings` 中已绑定的具体类型，未绑定的保留原样。递归处理 `List`、`Map`、`Set`、`Ref`、`Optional`、`Variadic`、`Fn`、`AppliedStruct` 等复合类型。
- 新增 `contains_type_var(&self) -> bool`：判断注解中是否包含 `TypeVar`，用于快速跳过不需要泛型解析的路径。
- `extract_generics_from_hint_form` 中增加对 `"generics"` 关键字的识别（原来只识别 `"type-vars"`）。

### `src/runner/preprocess.rs`

- 新增 `resolve_generic_return_type(fn_info, call_args, scope_types)`：当被调函数的返回类型包含 TypeVar 时，用 `matches_with_bindings` 将实参类型与形参注解匹配以收集绑定，然后用 `substitute_type_vars` 替换返回类型中的 TypeVar。
- `infer_type_from_expr` 中 4 个返回 `info.return_type.clone()` 的路径（Import evaled、Import code、Symbol lookup、直接 Fn head）在检测到 TypeVar 返回类型时，优先尝试 `resolve_generic_return_type`。
- `check_function_return_type` 中，当声明的返回类型包含 TypeVar 时跳过定义体校验（泛型返回类型只在调用处才可确定）。

### `calcit/test-types-inference.cirru`

- 新增 `test-generics-identity` 测试：验证 `identity 42` 推导为 `:number`、`identity |hello` 推导为 `:string`。

## 验证

- `cargo test` 全部通过
- `cargo clippy -- -D warnings` 无警告
- `yarn check-all` 全量集成测试通过
- `&inspect-type` 输出确认 `'n => number`、`'s => string`
