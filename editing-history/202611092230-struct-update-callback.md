# Struct 上 `update` 的回调拿到字段类型（#2647）

## 决策

- `update` 的 checked call contract 增加 Struct 接收者：字段参数是 Tag 字面量且字段存在时，期望类型为 `[Struct, Tag, Field -> Field]`，返回原 Struct 类型。原来字段类型只在调用检查阶段（`specialize_update_expected_types`）使用，回调预处理时没有上下文，参数是 Dynamic，报 `E_CALL_ARGUMENT_UNPROVEN`。
- 字段不是 Tag 字面量、字段不存在，或字段类型含 Struct 自身的类型参数（如 `Box` 的 `:value T`）时不给合同，沿用原有的字段检查，避免回调去决定这个参数（review）。
- `update-in` 进入 Struct 字段已有明确规则（提示结束路径、改用 `update`），不在本次放开。方法写法 `.update` 在 Struct 上不存在，也不新增。

## 验证

- `tests/struct_update_callback_cli.rs`：`str x` 通过；`+ 1 x` 报 `+` 第 2 个参数得到 String；返回 Number 时报 updater 应为 `fn(:string) -> :string`。
- `cargo test --lib`、core `test --tag unit`、`--test strict_check_cli --test fix_cli --test post_lowering_cli`。
