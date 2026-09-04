# Specialize collection key and mutation contracts / 专门化集合键与修改契约

## Context / 背景

The public `contains?` and `assoc` facades used independent generic variables for their receiver, key, and value. Those annotations looked typed but did not express that a `Map<K,V>` must receive `K`/`V`, that a List uses Number indices and preserves its item type, or that Set containment uses its member type. Invalid calls therefore passed preprocessing even when the receiver already provided exact evidence.

公共 `contains?` 与 `assoc` facade 为 receiver、key 和 value 使用彼此独立的泛型变量。它们表面上已有类型，却没有表达 `Map<K,V>` 必须接收 `K`/`V`、List 使用 Number 索引并保持成员类型，或者 Set containment 使用自身成员类型；因此即使 receiver 已提供精确证据，错误调用仍会通过预处理。

## Change / 修改

- Extend core call-site specialization to recover `contains?` contracts for List/String/Enum indices, Map keys, and Set members.
- Recover `assoc` contracts for List indices/items, Map keys/values, statically known Struct field values, and Enum payload indices. Enum replacement values remain open when no precise variant/slot evidence exists because payloads may be heterogeneous.
- Preserve the compatibility path for Dynamic or unsupported receivers rather than inventing unavailable relationships.
- Align Rust primitive metadata so native Map association carries `Map<K,V>`, `K`, and `V`, while native Set membership carries `Set<T>` and `T`.
- Expand the existing Snapshot fixture from five to fourteen mismatch cases and add unit assertions for the recovered relationships.
- Keep the bundled-core Dynamic inventory unchanged at `schemaDynamic=278`, `unresolved=184`, and `typeNotFull=134`; this tranche enforces erased relationships rather than rewriting schemas cosmetically.

- 扩展 core 调用点专门化：`contains?` 对 List/String/Enum 恢复索引类型，对 Map 恢复键类型，对 Set 恢复成员类型。
- `assoc` 对 List 恢复索引/成员关系，对 Map 恢复键/值关系，按静态 Struct 字段约束新值，并要求 Enum payload index 为 Number。Enum payload 可异构，因此缺少精确 variant/slot evidence 时 replacement value 仍保持开放。
- Dynamic 或当前无法可靠表示的 receiver 继续保留兼容路径，不虚构缺失的关系。
- 同步 Rust primitive metadata：native Map assoc 携带 `Map<K,V>`、`K`、`V`，native Set membership 携带 `Set<T>`、`T`。
- 将既有 Snapshot fixture 从五类错误扩展到十四类，并增加关系恢复的单元断言。
- bundled-core Dynamic inventory 保持 `schemaDynamic=278`、`unresolved=184`、`typeNotFull=134`；本批目标是执行被擦除的关系，而不是装饰性改写 schema。

## Validation / 验证

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test` (672 lib, 290 CLI, 23 WASM)
- `yarn check-all` (Agent interface 18/18, bundled core 237/237, classification 278/278, native/JS/IR/WASM/performance)
