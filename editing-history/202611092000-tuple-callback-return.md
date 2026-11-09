# lambda 尾部 `[]` 按期望元组检查（#2645）

## 决策

- 函数返回检查在声明类型带元组时，取尾部表达式的位置证据，而不是 `[]` 推断出的 `List Dynamic`：
  - 声明 `Tuple …`：尾部 `[]` 字面量（已有规则）。
  - 声明 `Fn … (-> (Tuple …))`：尾部是不写 `hint-fn` 的 lambda，用它的尾部 `[]` 替换推断的返回类型（复用 #2561 的 `callback_tail_tuple_evidence`）。
  - 声明 `List (Tuple …)`：尾部是降低后的 `&list:map` 调用且回调是不写 `hint-fn` 的 lambda，结果按 `List (Tuple …)` 证明。
- 证据只来自字面量位置的类型，位置类型不符仍然报错，诊断里显示 `tuple<bool, string>` 这样的具体位置。
- 回调参数类型不变，写了 `hint-fn` 的回调保留显式签名。

## 验证

- `cargo test --test tuple_callback_return_cli --test map_callback_tuple_cli`：直接返回、返回 lambda、`.map` / `map`、`let` 链尾部都通过；交换位置后五处都报错。
