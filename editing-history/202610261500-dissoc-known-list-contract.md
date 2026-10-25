# 已知 List 上的公开 dissoc 保留 List 合同

#2388 让已知 Map 上的公开 `dissoc` 降为 `&map:dissoc`。生态复测后 calcium-workflow 剩 2 条诊断，
都来自 List：`dissoc extended-order 1` 的结果仍是 Dynamic，随后 `reverse` 报
`E_ERASED_GENERIC_RELATION` / `E_CALL_ARGUMENT_UNPROVEN`。

修正：在同一张接收者类型特化表加 `("dissoc", List) => &list:dissoc`，只在恰好一个下标且下标
已证明为 Number 时生效，返回推断保留 `List<T>`。多下标、未证明为 Number 的下标与开放接收者
继续走公开定义及其报错。

回归：`strict_public_dissoc_keeps_a_known_collection_contract`（原 Map 用例改名并扩充）增加
List 的 Number 下标通过、Dynamic 下标仍被拒绝；修前 Number 下标用例失败。core 定义测试
native 749/749、JS 747/747、WASM 481/481 通过。生态复测：calcium-workflow 21→0。

English: public `dissoc` on a List with exactly one proven Number index now lowers to
`&list:dissoc`, keeping `List<T>` under strict checking. Other shapes keep the generic definition.
