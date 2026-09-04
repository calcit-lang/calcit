# Specialize sort comparator contracts / 专门化 sort 比较器契约

`sort` and `&list:sort` now preserve `List<T>` and require their optional comparator to satisfy `T, T -> Number`. Call-site specialization recovers the concrete member type for diagnostics, while one-argument natural sorting and Syntax-typed macro collections keep their existing behavior.

`sort` 与 `&list:sort` 现在会保留 `List<T>`，并要求可选 comparator 满足 `T, T -> Number`。调用点专门化会恢复具体成员类型用于诊断；单参数自然排序与 Syntax 类型的 macro collection 保持原有行为。

The type-fail fixture covers both public and native sort paths, rejecting comparators over the wrong member type and callbacks with the wrong arity. The core snapshot schemas, inference tests, migration guidance, and static-analysis reference are updated together.

type-fail fixture 同时覆盖公开与原生 sort 路径，拒绝成员类型错误的 comparator 和元数错误的 callback。core snapshot schema、推断测试、迁移指南与静态分析文档同步更新。

Validation: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, serialized `cargo test --all-targets --all-features` (689 library, 294 CLI, 23 WASM tests), and `yarn check-all` all pass. The core quality inventory remains at 278 schema-Dynamic, 184 unresolved, and 134 not-full positions.

验证：`cargo fmt --all`、`cargo clippy --all-targets --all-features -- -D warnings`、串行 `cargo test --all-targets --all-features`（689 个 library、294 个 CLI、23 个 WASM 测试）与 `yarn check-all` 全部通过。core quality inventory 仍为 278 个 schema-Dynamic、184 个 unresolved 与 134 个 not-full 位置。
