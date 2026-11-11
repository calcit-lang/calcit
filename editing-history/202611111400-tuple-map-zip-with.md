# 同构 Tuple 的 map 保持长度与 `zip-with`

背景：0.42.0 第 8 项（F3）。生态向量运算用 `List Number`，加 `.unwrap $ nth v n`；改成 `Tuple Number Number Number` / `Vec3` 别名后（D6 已在 #2694 修复），`map` 结果仍退化成 `List`，向量加法也没有对应写法（下标非字面量时 `nth` 报 `E_TUPLE_INDEX`）。

决定：

- `map` / `.map` / `map-indexed` 的接收者是同构 Tuple（含别名）时，返回同长度的 `Tuple U …`（#2702，PR #2703）。`filter`、`concat` 等改变长度的操作和异构 Tuple 不变。
- 新增核心函数 `zip-with xs ys f`（Jon 2026-11-11 选择，代替“在 map-indexed 回调里证明 `nth b i` 不越界”的特例规则）。运行时取较短长度；类型上两个长度相同的同构 Tuple 返回同长度 Tuple，否则 `List U`。

实现位置：`checked_call_contract.rs` 的 `resolve_checked_call_contract`（Tuple 长度、`zip_with_contract`），`type_inference.rs` 对降级后的 `&list:map` 重新取 `map` 契约，`preprocess/mod.rs` 的内建方法候选把 Tuple 视同 List。

验证：`cargo test --test tuple_map_length_cli`。
