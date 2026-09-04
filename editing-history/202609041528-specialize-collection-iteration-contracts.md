# Specialize collection iteration contracts / 专门化集合迭代契约

Receiver-driven callback checking now covers the remaining public iteration facades `any?`, `every?`, and `each` in addition to `filter`. List and Set callbacks receive their concrete member type; Map callbacks receive the reliable `List<Dynamic>` shape of the heterogeneous runtime `[key value]` pair.

receiver-driven callback 检查现在从 `filter` 扩展到其余公共迭代 facade：`any?`、`every?` 与 `each`。List 和 Set callback 接收具体成员类型；Map callback 接收异构运行时 `[key value]` pair 的可靠 `List<Dynamic>` 外形。

`any?` and `every?` require callbacks to return Bool. `each` constrains only the callback input and intentionally accepts any return type because the operation discards callback results and returns Unit. The negative fixture covers all three policies without tightening the open global facades dishonestly.

`any?` 与 `every?` 要求 callback 返回 Bool；`each` 只约束 callback 输入，并有意允许任意返回类型，因为该操作丢弃 callback 结果并返回 Unit。负向 fixture 覆盖三类策略，同时不对开放的全局 facade 做不诚实收紧。

Validation: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, serialized `cargo test --all-targets --all-features` (677 library, 290 CLI, 23 WASM tests), and `yarn check-all` all pass. The core quality inventory remains at 278 schema-Dynamic, 184 unresolved, and 134 not-full positions.

验证：`cargo fmt --all`、`cargo clippy --all-targets --all-features -- -D warnings`、串行 `cargo test --all-targets --all-features`（677 个 library、290 个 CLI、23 个 WASM 测试）与 `yarn check-all` 全部通过。core quality inventory 仍为 278 个 schema-Dynamic、184 个 unresolved 与 134 个 not-full 位置。
