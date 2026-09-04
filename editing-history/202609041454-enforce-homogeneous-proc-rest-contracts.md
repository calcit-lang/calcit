# Enforce homogeneous proc rest contracts / 强制原生 proc 同质 rest 契约

Typed `Variadic<T>` now has an explicit per-proc checking policy. `&list:concat`, `&merge`, and `&map:dissoc` validate every remaining argument against the generic type established by earlier arguments, while `[]` and `#{}` continue to use the annotation only as common-member inference evidence and preserve heterogeneous fallback to `Dynamic`.

现在为 typed `Variadic<T>` 增加了明确的逐 proc 检查策略。`&list:concat`、`&merge` 与 `&map:dissoc` 会用前序参数建立的泛型绑定检查全部剩余参数；`[]` 与 `#{}` 则仍只把该标注用于公共成员类型推断，并保留异构字面量回退到 `Dynamic` 的兼容行为。

Type-checking diagnostics now retain concrete List, Map, Set, and Ref payloads. A rejected concat or merge therefore reports distinctions such as `list<number>` versus `list<tag>` instead of the unhelpful `:list` versus `:list`. The type-fail fixture, Rust policy tests, and migration documentation cover both behaviors.

类型检查诊断现在会保留 List、Map、Set 与 Ref 的具体载荷。被拒绝的 concat 或 merge 因而会显示 `list<number>` 与 `list<tag>` 之类的差异，不再输出无效的 `:list` 对 `:list`。type-fail fixture、Rust 策略测试与迁移文档同时覆盖了这两类行为。

Validation: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, serialized `cargo test --all-targets --all-features` (676 library, 290 CLI, 23 WASM tests), and `yarn check-all` all pass. The core quality inventory remains at 278 schema-Dynamic, 184 unresolved, and 134 not-full positions.

验证：`cargo fmt --all`、`cargo clippy --all-targets --all-features -- -D warnings`、串行 `cargo test --all-targets --all-features`（676 个 library、290 个 CLI、23 个 WASM 测试）与 `yarn check-all` 全部通过。core quality inventory 仍为 278 个 schema-Dynamic、184 个 unresolved 与 134 个 not-full 位置。
