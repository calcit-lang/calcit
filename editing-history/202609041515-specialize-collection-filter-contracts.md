# Specialize collection filter contracts / 专门化集合 filter 契约

The public `filter` compatibility facade keeps its open global schema because one annotation cannot honestly relate List, Set, and heterogeneous Map entries. When the receiver is concrete, preprocessing now reconstructs the real predicate contract: `List<T>` and `Set<T>` require `T -> Bool`; Map requires `List<Dynamic> -> Bool` because iteration passes a heterogeneous `[key value]` pair.

公共 `filter` 兼容 facade 继续保留开放的全局 schema，因为单一标注无法诚实表达 List、Set 与异构 Map entry 的全部关系。当 receiver 类型已知时，预处理现在会恢复真实 predicate 契约：`List<T>` 与 `Set<T>` 要求 `T -> Bool`；Map 要求 `List<Dynamic> -> Bool`，因为迭代时传入的是异构 `[key value]` pair。

This deliberately preserves the pair's reliable List shape without pretending that Map key and value types are equal. A type-fail fixture proves that Number-returning predicates are rejected for all three receiver families, while the existing positive core calls remain valid.

该设计保留 pair 可靠的 List 外形，但不会伪造 Map key 与 value 类型相同。type-fail fixture 证明三类 receiver 都会拒绝返回 Number 的 predicate，同时既有 core 正向调用保持有效。

Validation: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, serialized `cargo test --all-targets --all-features` (677 library, 290 CLI, 23 WASM tests), and `yarn check-all` all pass. The core quality inventory remains at 278 schema-Dynamic, 184 unresolved, and 134 not-full positions.

验证：`cargo fmt --all`、`cargo clippy --all-targets --all-features -- -D warnings`、串行 `cargo test --all-targets --all-features`（677 个 library、290 个 CLI、23 个 WASM 测试）与 `yarn check-all` 全部通过。core quality inventory 仍为 278 个 schema-Dynamic、184 个 unresolved 与 134 个 not-full 位置。
