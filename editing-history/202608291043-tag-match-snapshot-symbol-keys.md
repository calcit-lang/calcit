# `tag-match` deprecation and Snapshot Symbol identifier keys

## 中文概要

- 将 `calcit.core/tag-match` 标记为普通 `:deprecated`，迁移说明指向原生 `match`；废弃调用继续进入 `analyze deprecated` 与 quality 的 `deprecatedCalls`。
- 将 core 内 18 个 Option/Result、collection destructuring 与 IO helper 的实际调用迁移到 `match`，并同步迁移相关 examples 和 definition-attached tests；core deprecated 分析归零。
- 为保留的兼容宏补充 validated pattern-list 类型断言，使专门的 legacy runtime test 不产生类型告警。
- Snapshot runtime、format migration、detailed snapshot 和 build-script reader 对 `:files` namespace key、`:defs` definition key 同时接受旧 String 与新 Symbol。
- Snapshot writer 只输出 Symbol identifier key；String/Symbol 归一化重名时明确报错，避免 HashMap 静默覆盖。
- core Snapshot 经新版 writer 完成 Symbol key 规范化；build.rs 同步支持该格式。

## English summary

- Marked `calcit.core/tag-match` with the ordinary `:deprecated` tag and directed migration to native `match`; calls remain part of deprecated analysis and the quality `deprecatedCalls` budget.
- Migrated 18 real core Option/Result, collection-destructuring, and IO-helper call sites to `match`, including related examples and definition-attached tests; core deprecated analysis now reports zero calls.
- Added validated pattern-list type assertions inside the retained compatibility macro so its dedicated legacy runtime test remains warning-free.
- Made runtime, format-migration, detailed-snapshot, and build-script readers accept both legacy String and canonical Symbol namespace/definition keys.
- Made Snapshot writers emit only Symbol identifier keys and reject normalized String/Symbol collisions instead of silently overwriting entries.
- Canonicalized the bundled core Snapshot to Symbol keys and updated `build.rs` to consume the format.

## Verification notes

- Rust library tests: 580 passed.
- Core attached tests: 223 passed.
- `analyze deprecated --deps`: 0 core calls after migration.
- Temporary legacy-String Snapshot formatted to Symbol keys and reloaded through `query context`.
- Full Rust, strict clippy, JS/IR/WASM, and Agent interface checks passed before commit/PR.
