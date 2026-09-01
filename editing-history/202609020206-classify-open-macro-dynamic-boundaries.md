# Classify open macro Dynamic boundaries / 区分开放宏 Dynamic 边界

## Summary / 概要

- Added the `intentional-macro-syntax` weak-type intent for `Dynamic` nested specifically in strict `MacroSignature` `Expr<...>` inputs, rest inputs, and expression expansions.
- Kept whole-Dynamic macro schemas, `MacroExpansionType::Dynamic`, and `Definition<Dynamic>` classified as unresolved.
- Suppressed unresolved-schema warnings only for the reviewed `Expr<Dynamic>` boundary; the occurrences remain visible in weak-type reports and continue to make type coverage partial.
- Kept intentional macro syntax in the per-definition `schemaDynamic` quality budget, so adding new open macro positions still requires a reviewed baseline update.
- Reclassified 94 bundled-core positions: total schema-Dynamic inventory remains 297, while unresolved debt falls from 297 to 203.

## 知识点

- 宏的语法阶段契约与表达式的语义类型是两层约束：`Expr<Dynamic>` 仍检查“这里必须是表达式”，只是表达式结果类型有意开放。
- 不能把所有 macro 内部的 `Dynamic` 都视为同一种边界；`Dynamic` expansion 和 `Definition<Dynamic>` 仍会绕过更关键的阶段/定义约束，因此继续作为 unresolved。
- “不计入 unresolved”不等于“不可见或不受约束”。`weak-types` 保留完整位置，coverage 仍为 partial，quality gate 仍锁住 `schemaDynamic` 总量。

## Validation / 验证

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface` (18/18)
- `yarn check-all`
- Bundled-core quality baseline: `schemaDynamic=297`, `unresolved=203`, zero deltas after regeneration.
