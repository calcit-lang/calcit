# Check typed rest arguments / 检查 typed rest 参数

## Context / 背景

Function schemas preserved `:rest` metadata for body inference and higher-order signatures, but direct calls only checked fixed arguments. The public `dissoc` facade therefore could not enforce that every List index is a Number or every Map key has the receiver's `K` type. Proc checking likewise stopped at `Variadic<T>`, and `&map:dissoc` metadata erased its Map key/value relationship.

函数 schema 已为函数体推断和高阶签名保留 `:rest` metadata，但直接调用只检查固定参数。因此公共 `dissoc` facade 无法保证每个 List index 都是 Number，或每个 Map key 都符合 receiver 的 `K` 类型。Proc 检查也会在 `Variadic<T>` 处停止，而 `&map:dissoc` metadata 还擦除了 Map 的键值关系。

## Change / 修改

- Feed named and local function rest types into the shared argument checker, including rest-only functions and runtime functions that carry both trailing `Variadic<T>` and separate rest metadata.
- Preserve generic bindings across fixed and rest arguments, substitute bound variables in diagnostics, and run trait-bound checks after variadic arguments.
- Specialize public `dissoc` from its receiver: List rest arguments are Number indices and Map rest arguments are `K` keys. Set and Struct are intentionally excluded because they do not expose a valid `:dissoc` method contract.
- Align `&map:dissoc` Rust metadata and bundled-core schema to `Map<K,V>, K, &K -> Map<K,V>`, then enable concrete proc rest checking for that operation.
- Keep legacy heterogeneous proc constructors on their compatibility path. Enforcing every existing `Variadic<T>` immediately makes `[]` homogeneous and creates broad migration noise, so that semantic change remains a separate tranche.
- Extend the collection mismatch Snapshot and tests for first/later public rest keys, the native variadic key path, and typed local rest functions.

- 将 named/local function 的 rest 类型接入共享参数检查，包括纯 rest 函数，以及同时携带尾部 `Variadic<T>` 和独立 rest metadata 的运行时函数。
- 固定参数与 rest 参数共享泛型绑定；诊断显示替换后的具体类型，并在 variadic 参数之后继续验证 trait bound。
- 根据 receiver 专门化公共 `dissoc`：List 的 rest 参数必须是 Number index，Map 的 rest 参数必须是 `K` key。Set 与 Struct 没有有效的 `:dissoc` method contract，因此明确排除。
- 将 `&map:dissoc` 的 Rust metadata 与 bundled-core schema 对齐为 `Map<K,V>, K, &K -> Map<K,V>`，并为该操作启用明确的 proc rest 检查。
- legacy 异构 proc constructor 继续走兼容路径。立即强制所有既有 `Variadic<T>` 会把 `[]` 变成同质构造器并产生大范围迁移噪音，因此该语义变化留作独立批次。
- 扩展 collection mismatch Snapshot 与测试，覆盖公共调用的首个/后续 rest key、原生 variadic key 路径，以及 typed local rest function。

## Validation / 验证

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test -- --test-threads=1` (674 lib, 290 CLI, 23 WASM)
- `yarn check-all` (Agent interface 18/18, bundled core 237/237, classification 278/278, native/JS/IR/WASM/performance)

The bundled-core inventory remains `schemaDynamic=278`, `unresolved=184`, and `typeNotFull=134`.

bundled-core inventory 保持 `schemaDynamic=278`、`unresolved=184` 与 `typeNotFull=134`。
