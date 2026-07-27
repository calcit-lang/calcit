# TypeSlot + Enum 编译期安全增强

## 概要

让 `deftype-slot` / `bind-type` 机制真正实现编译期 enum variant 校验。此前 TypeSlot 绑定的 enum 在自动推导 (`:: → %::`) 路径上缺乏变体验证，导致拼写错误或不存在的 variant 无法在 `cr js` / `--check-only` 时报出。

## 修改的文件

### `src/calcit/type_annotation.rs`

- `resolve_to_enum_with_ref()`: 新增 `TypeSlot` 分支，通过 `resolve_type_slot()` 解析绑定类型后委托
- `matches_with_bindings()`: 双向 `Tuple/Enum` 匹配 — 原先只有 `(Tuple, Enum)` 方向，TypeSlot 解析后顺序会翻转，新增 `(Enum, Tuple)` 方向

### `src/calcit.rs`

- 将 `resolve_type_slot` 加入 `pub use` 导出列表

### `src/runner/preprocess.rs`

- `resolve_enum_value()`: 在 `as_struct()` 调用前增加 TypeSlot 解析（避免 `&CalcitStruct` 生命周期问题）
- 新增 `try_rewrite_local_fn_tuple_args_to_enum_tuples()`: 对本地函数调用（如回调 `d!`）也执行 tuple→enum 自动重写
- 主预处理 `Calcit::Local` 分支: 增加 local fn 调用的重写+类型检查（clone 避免借用冲突）
- `try_rewrite_single_tuple_to_enum_tuple()`: 增加变体验证 — 重写后立即检查 tag 是否是 enum 的合法 variant，不合法则发出警告

### `src/bin/cr_tests/type_fail.rs`

- 新增 `type_fail_type_slot_enum_invalid_variant` 测试，验证 TypeSlot 绑定的 enum 在自动重写路径下能正确检测不存在的 variant

### `calcit/type-fail/type-slot-enum-invalid-variant.cirru`

- 新增测试 fixture：声明 `defenum Action`、通过 TypeSlot 绑定、调用 `takes-action $ :: :nonexistent |hello`，期望产生 variant 不存在警告

## 知识点

1. **TypeSlot 解析链**: `TypeSlot(name) → resolve_type_slot(name) → CalcitTypeAnnotation (通常是 TypeRef)` → 再通过 `resolve_to_enum_with_ref()` 得到 enum 定义。这是一个两步间接过程。

2. **`matches_with_bindings` 双向性**: TypeSlot 被解析后会调 `bound.matches_with_bindings(other, bindings)`，此时 self/other 顺序可能翻转。所有组合型匹配（如 Tuple/Enum）都需要双向 pattern。

3. **`as_struct()` 返回引用**: `CalcitTypeAnnotation::as_struct()` 返回 `Option<&CalcitStruct>`，无法从临时解析结果中返回引用。修复方案是在调用点先解析 TypeSlot 再调 `as_struct()`。

4. **自动重写的 `%::` 不经过 `preprocess_expr`**: `try_rewrite_single_tuple_to_enum_tuple` 构造的 `[NativeEnumTupleNew, ...]` 列表是在预处理完参数后生成的，不会经过 `check_proc_arg_types` 中的 `check_enum_tuple_construction`。因此变体验证必须在重写函数内部完成。

5. **Local fn 调用缺少自动重写**: 原先只有 `CalcitFn` 调用走 `try_rewrite_tuple_args_to_enum_tuples`。`Calcit::Local` 调用（如回调参数 `d!`）需要单独处理。新函数接受 `CalcitFnTypeAnnotation` 而非 `CalcitFn`。

6. **Lambda-in-map 的 EXPECTED_FN_TYPE 盲区**: `fn (e d!)` 在 hashmap 值（如 `:on-click`）中时，`d!` 不会获得 `EXPECTED_FN_TYPE` 注入的类型信息。目前通过显式 `%:: Op :variant` 写法绕过，属于已知架构限制。
