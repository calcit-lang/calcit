# 2026-03-07 schema.args 类型检查接通与 fn 类型注解修复

## 背景

上一次 session（commit a008f8b）为 calcit-core 的 54+ 函数添加了 `:schema` 中的 `:args` 字段。但发现这些 schema
并未实际触发调用现场的类型检查——`CalcitFn.arg_types` 只从函数体的 `assert-type` 扫描，从未读取 schema `:args`。

## 根因分析

类型检查数据流：
```
calcit-core.cirru :schema :args
  → preprocess.rs 注入为 (HintFn schema_value) 进 defn 体
  → syntax.rs defn 调用 detect_arg_type_hints → collect_arg_type_hints_from_body
  → 只扫描 assert-type/assert-traits，忽略 HintFn！
  → CalcitFn.arg_types 对 schema-only 函数全为 Dynamic
  → check_user_fn_arg_types 提前返回，无任何检查
```

## 修复内容

### 1. 接通 schema.args → CalcitFn.arg_types（`syntax.rs` + `type_annotation.rs`）

- 新增 `CalcitTypeAnnotation::extract_arg_types_from_hint_form(form, params)` 方法
- 新增 `parse_schema_args_types` 和 `is_args_list_head` 辅助方法
- `syntax.rs:defn` 中优先从 hint-fn（schema 注入）提取 arg_types，再回退到 assert-type 扫描

### 2. fn 类型注解解析修复（`type_annotation.rs`）

发现 `(:: :fn ([] 'T 'U) (:: 'T 'U) 'T)` 格式的 fn 类型注解被错误解析，因为 `[]` 头部（list constructor proc）
出现在 args 列表中被当作普通类型处理：

- **三处** `parse_type_annotation_form` `:fn` 分支：args_form 为 List 时跳过 `[]` 头部
- **`parse_generics_list`**：跳过 `[]` 头部，使 `([] 'T 'U)` 作为合法 generics 列表（`'T 'U` 不能作 Cirru EDN operator）
- **`matches_signature`**：移除 generics 数量检查，改用 `matches_with_bindings` 解析 TypeVar 绑定（使具体函数类型能匹配泛型签名）
- **`matches_with_bindings`**：`Tag` 可满足 `DynFn`/`Fn` 约束（Calcit 中 `:tag` 作为 map key accessor 可调用）

### 3. method 调用类型检查修复（`preprocess.rs`）

方法调用的 arg 类型检查循环中缺少对 `Variadic` 的处理，导致 `.union s1 s2` 误报。修复为与 `check_user_fn_arg_types` 一致的 Variadic 处理。

### 4. calcit-core.cirru schema 修正

| 函数         | 修正内容 |
|------------|------|
| `every?`   | `:args $ (:: :list :dynamic)` → `:dynamic`（接受 set） |
| `keys`     | `:args $ :map` → `:dynamic`（接受 record） |
| `merge`    | `:args $ :map` + `:rest $ :map` → `:dynamic`（接受 record） |
| `merge-non-nil` | 同上 |
| `reduce`   | fn-arg 格式 `([] 'T 'U)(:: 'T 'U)` → `([] 'T 'U)([] 'T 'U)`（正确 generics+args） |
| `map`      | fn-arg 格式 `([] 'T 'U)(:: 'T)` → `([] 'T 'U)([] 'T)`（正确 generics+1-arg） |

## 验证

- `each ([] 1 2 3) 42` → 正确警告（42 不是 :fn）
- `reduce ([] 1 2 3) 0 &+` → 无误报（&+ 满足 fn(T,U)->T）
- `cargo test`：92 passed, 0 failed（之前 1 个失败的 test_examples_field_parsing 也通过了）
- `yarn check-all`：全量集成测试通过，0 warnings

## 关键知识点

- `([] 'T 'U)` 是合法 Cirru EDN（`[]` 是合法 operator），`('T 'U)` 不合法（`'T` 不能作 operator）
- fn 类型 generics 列表格式：`([] 'T 'U)` 跳过 `[]` 头后得到 `[T, U]`
- fn 类型 args 列表格式：`([] :number :number)` 跳过 `[]` 头后得到 `[:number, :number]`
- `matches_signature` 不应检查 generics 数量——调用方传入的具体函数不声明 generics，但 TypeVar 绑定会在 args 匹配时解析
