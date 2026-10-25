# 已知 Map 上的公开 dissoc 保留 Map 合同

公开 `dissoc` 同时接受 List 和 Map，schema 只能写成 `Dynamic -> Dynamic`。#2187 之后，
strict 不再从 kind 守卫推出结果类型，于是 `reset! *cache $ dissoc @*cache k` 这类写法在
生态库里出现 `W_RESET_ARG_TYPE_MISMATCH` / `E_CALL_ARGUMENT_UNPROVEN`（memof、walnut-log）。
`assoc` 早就在已知接收者上按类型特化为 `&map:assoc` 等原生 proc，`dissoc` 缺了这一项。

修正：在已有的接收者类型特化表里加 `("dissoc", Map) => &map:dissoc`，只在至少有一个 key
时生效。`&map:dissoc` 的返回推断保留 `Map<K,V>`，key 仍按 Map key 合同检查。零 key 的报错、
List 下标检查与 Dynamic 接收者继续走原来的公开定义。

回归：`strict_public_dissoc_keeps_a_known_map_contract` 在 strict 下覆盖单 key、多 key 通过，
以及错误 key 类型仍被拒绝（修前单 key 用例失败）；core `dissoc` 已有的 13 个定义测试在
native/JS/WASM 通过。生态里 walnut-log 的 strict 诊断因此归零。

English: public `dissoc` on a receiver statically known to be a Map now lowers to `&map:dissoc`
through the existing receiver-type specialization table (as `assoc` already did), so the result
keeps its `Map<K,V>` contract under strict checking. Zero-key calls, List receivers and open
receivers keep the generic definition and its errors.
