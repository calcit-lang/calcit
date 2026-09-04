# Specialize collection map contracts / 专门化集合映射契约

Public `map` calls now recover receiver-driven mapper contracts that the compatibility facade cannot express. List and Set receivers require `T -> U`; Map receivers require the runtime pair shape `List<Dynamic> -> List<Dynamic>`. Known Set receivers also lower directly to `&set:map`.

公共 `map` 调用现在会恢复兼容 facade 无法表达的 receiver-driven mapper 契约。List 与 Set receiver 要求 `T -> U`；Map receiver 要求运行时 pair 外形 `List<Dynamic> -> List<Dynamic>`。已知 Set receiver 也会直接 lowering 到 `&set:map`。

Syntax collections used during macro expansion remain on the open facade because their nested AST shape is not modeled as a runtime collection member type. Negative fixtures cover List, Set, and Map callback mismatches, while unit coverage locks both the Set lowering and this phase-safe fallback.

宏展开期间使用的 syntax collection 继续走开放 facade，因为其嵌套 AST 外形并不是已建模的运行时集合成员类型。负向 fixture 覆盖 List、Set 与 Map callback mismatch，单元测试同时固定 Set lowering 和这项 phase-safe 回退。

Validation: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, serialized `cargo test --all-targets --all-features` (680 library, 290 CLI, 23 WASM tests), and `yarn check-all` all pass. The core quality inventory remains at 278 schema-Dynamic, 184 unresolved, and 134 not-full positions.

验证：`cargo fmt --all`、`cargo clippy --all-targets --all-features -- -D warnings`、串行 `cargo test --all-targets --all-features`（680 个 library、290 个 CLI、23 个 WASM 测试）与 `yarn check-all` 全部通过。core quality inventory 仍为 278 个 schema-Dynamic、184 个 unresolved 与 134 个 not-full 位置。
