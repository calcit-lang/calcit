# Specialize collection fold contracts / 专门化集合 fold 契约

Known List, Set, and Map receivers now recover the runtime `foldl`/`reduce` reducer relationship at the call site. An initial accumulator `U` requires a reducer `U, T -> U`; Map iteration uses the honest heterogeneous pair shape `List<Dynamic>`. Syntax-typed macro collections retain their phase-aware open path.

已知 List、Set 与 Map receiver 现在会在调用点恢复运行时 `foldl`/`reduce` reducer 关系。初始 accumulator `U` 要求 reducer 为 `U, T -> U`；Map 迭代使用诚实的异构 pair 外形 `List<Dynamic>`。syntax-typed macro collection 保留 phase-aware 开放路径。

Direct native `foldl` calls preserve the initial accumulator as their result type only when the reducer has a concrete compatible function signature; `DynFn` remains Dynamic instead of gaining unsupported evidence. Negative fixtures cover List/Set member mismatches and the Map pair mismatch.

直接 native `foldl` 调用只在 reducer 具有具体且兼容的函数签名时，才把初始 accumulator 保留为结果类型；`DynFn` 继续保持 Dynamic，不会获得缺乏证据的类型。负向 fixture 覆盖 List/Set 成员 mismatch 与 Map pair mismatch。

Validation: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, serialized `cargo test --all-targets --all-features` (682 library, 290 CLI, 23 WASM tests), and `yarn check-all` all pass. The core quality inventory remains at 278 schema-Dynamic, 184 unresolved, and 134 not-full positions.

验证：`cargo fmt --all`、`cargo clippy --all-targets --all-features -- -D warnings`、串行 `cargo test --all-targets --all-features`（682 个 library、290 个 CLI、23 个 WASM 测试）与 `yarn check-all` 全部通过。core quality inventory 仍为 278 个 schema-Dynamic、184 个 unresolved 与 134 个 not-full 位置。
