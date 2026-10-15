# 开放实参的回调上下文保留 Dynamic 绑定（#2086）

## 依据与设计

`tagging-edn` 用 `map?` 收窄 Dynamic 后调用 `filter-map-kv`，普通检查和运行都通过，
strict workflow 却报 `E_CALL_ARGUMENT_UNPROVEN`。最小复现不依赖 tagging：任何
`if (map? data) (filter-map-kv data $ fn (k v) ...)` 都会失败。

原因在 #2015 引入的过滤：前序实参证明 `K`、`V` 绑定为裸 Dynamic 后，这些绑定被丢弃，
回调参数上下文于是残留被调方的原始泛型名 `'K`、`'V`。审计证明把这些名字当作调用方的刚性变量，
回调的 `fn('K, 'V) -> MapEntryDecision<...>` 因此无法证明 `fn(K, V) -> MapEntryDecision<R, S>`。

本次删除这条过滤，保留已证明的 Dynamic 绑定。回调参数因而是显式开放值：可以保存、传递、包装，
具体使用时仍要求 decode/narrow，与开放值语义一致。不改兼容关系、方差、泛型刚性或诊断编号。

#2015 记录过保留裸 Dynamic 会让 tagging-edn 回调退化为缺少签名证据的 DynFn；
当前主线上 core 全定义检查和 `yarn check-all` 均通过，该退化不再出现。

## 验证

- `tests/fix_cli.rs` 新增 strict workflow 回归：core `tagging-edn` 与用户写的开放 Map 恒等回调；撤回修复时该测试失败。
- `yarn check-all`、`cargo test`、`cargo clippy -- -D warnings`。
