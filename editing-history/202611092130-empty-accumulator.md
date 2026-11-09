# 空集合累加初值按回调放宽（#2646）

## 决策

- fold 回调（proc `foldl`、方法 `.foldl` / `.fold`、core 函数 `fold` / `foldl` / `reduce`）的初值是空集合字面量、accumulator 带空槽（`List Never` 等）时，先按原上下文预处理回调；若回调返回的类型是初值的放宽（初值能证明为它、不含类型变量、没有空槽），丢弃第一遍的警告，用放宽后的类型作为 accumulator 和返回位置再预处理一次。之后的调用检查和返回推断沿用已有的 `specialize_collection_fold_expected_types`，回调参数已是放宽后的类型。
- `recursive_inputs` 里“开放 proc 参数不为空槽提供成员证据”的规则原来只在有尾部转移（`recur`）时生效；现在对所有空槽都生效。否则 `append acc x`（proc 签名是 `List Dynamic`）会先把空槽解成 `Dynamic`，回调返回 `List Dynamic`，放宽无从谈起。成员证据改由集合增长推断给出（`append (List Never) String` → `List String`）。
- 放宽到开放成员（如 Map pair 的 `List Dynamic`）也接受，结果与改动前一致。

## 验证

- `tests/empty_accumulator_cli.rs`：Set / List / Map 初值、前缀 `foldl`、`let` 内都通过；加入 Bool 时报返回类型 `set<bool>`。
- `cargo test --lib`、core `test --tag unit`（751 通过，含 `fold#left-to-right`、`map-list-kv`）、`--test strict_check_cli --test fix_cli --test post_lowering_cli --test map_callback_tuple_cli`。
