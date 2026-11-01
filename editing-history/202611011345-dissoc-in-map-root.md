# `dissoc-in` 保留已知 Map 根类型

#2506：Wanderlist 的 `dissoc-in data $ [] :groups id`（`data: Map<Tag,Dynamic>`）在默认证明审计下报 `E_FN_RETURN_UNPROVEN`，因为 `dissoc-in` 的公开 schema 是 `Fn Dynamic (List K) (-> Dynamic)`，结果一律是 Dynamic。

`infer_core_dissoc_in_return_type` 只在根类型为 `Map<K,V>`、路径是非空字面量列表时给出 `Map<K,V>`：

- 一步路径就是 `dissoc`，Map 保持 Map。
- 多步路径的实现是 `assoc data p0 (dissoc-in (&get-raw data p0) ps)`。首键缺失时下一步落到 `dissoc` 或 `assoc` nil，两者都会 raise，所以能返回时 `p0` 已在 Map 里；只需嵌套结果符合 `V`。`V` 为 `Dynamic` 直接成立，`V` 为 Map 时按同一规则递归。

不扩大到空路径、动态路径、List/Struct 载荷或一般的 `T -> T` 关系，这些仍走开放合同。没有改公开 schema、运行时实现或诊断编号；`docs/core-dynamic-classification.md` 的 `dissoc-in` 返回位置说明同步更新。

验证：`type_inference.rs` 单测 `dissoc_in_keeps_known_map_root`，以及 `calcit/type-fail/dissoc-in-map-root.cirru`（通过）和 `dissoc-in-list-payload.cirru`（仍报 `E_FN_RETURN_UNPROVEN`）两个证明审计 fixture，由 `proof_dissoc_in_keeps_known_map_root` 驱动。check-all 关闭默认审计，所以放在 Rust 侧而不是 `:tests`。
