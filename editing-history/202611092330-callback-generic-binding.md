# 回调写在前面时从后面的实参取泛型绑定（#2648）

## 决策

- 用户函数调用预处理前，原来只为“只出现在回调返回位置”的泛型（`output_only_callback_generics`）预先从普通实参收集绑定。现在也包括回调里出现、且在回调之后的普通实参里同样出现的泛型（`later_evidenced_callback_generics`），输入和返回位置都算。回调因此按源码顺序先预处理时，也能拿到 `Store` / `Op` 这样的具体类型。
- 后面实参是 Dynamic 或含类型变量时不收集，和原规则一致；回调在前、证据也只在回调自己身上时，仍由回调推断。
- issue 里“无法唯一确定时给出具体提示”没有在本次实现。

## 验证

- `tests/callback_generic_binding_cli.rs`：inline lambda 与 let 绑定的 lambda 都通过；`+ store op` 报 `+` 第 1 个参数得到 String，不再是 `E_CALL_ARGUMENT_UNPROVEN`。
- `cargo test --lib`、core `test --tag unit`、`--test strict_check_cli --test fix_cli --test post_lowering_cli`。
