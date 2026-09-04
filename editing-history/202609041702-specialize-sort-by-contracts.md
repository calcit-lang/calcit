# Specialize sort-by function selectors / 专门化 sort-by 函数选择器

The function-selector form of `&list:sort-by` now recovers the concrete `List<T>` member type at each call site and requires `T -> K`. This closes a caller-visible Dynamic gap used by the public List `.sort-by` method while leaving the key type generic.

`&list:sort-by` 的函数 selector 形式现在会在调用点恢复具体的 `List<T>` 成员类型，并要求 `T -> K`。这收紧了公开 List `.sort-by` 方法上的 Dynamic 缺口，同时保持 key 类型为泛型。

The runtime also accepts a Tag as a field/key selector. That compatibility path remains deliberately open instead of pretending the type system has a function-or-tag union. Syntax-typed members likewise retain their phase-aware fallback. Negative coverage verifies that a Number-only selector is rejected for `List<String>` with a concrete diagnostic.

运行时也接受 Tag 作为字段或键 selector。该兼容路径继续保持开放，不伪装类型系统已具备 function-or-tag union；Syntax 类型成员也保留 phase-aware fallback。负例验证 `List<String>` 会以具体诊断拒绝仅接受 Number 的 selector。

Validation: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, serialized `cargo test --all-targets --all-features` (690 library, 294 CLI, 23 WASM tests), and `yarn check-all` all pass. The core quality inventory remains at 278 schema-Dynamic, 184 unresolved, and 134 not-full positions.

验证：`cargo fmt --all -- --check`、`cargo clippy --all-targets --all-features -- -D warnings`、串行 `cargo test --all-targets --all-features`（690 个 library、294 个 CLI、23 个 WASM 测试）与 `yarn check-all` 全部通过。core quality inventory 仍为 278 个 schema-Dynamic、184 个 unresolved 与 134 个 not-full 位置。
