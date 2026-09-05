# Direct-quote macro migration bridge / Direct-quote 宏迁移桥

- In the isolated `calcit edit format` loader, recover a strict macro contract only for definitions decoded from legacy direct quotes. Preserve required, optional, and rest arity as `Syntax`, use `Expr<Dynamic>` for the expansion, and grant no capabilities.
- Keep the normal Snapshot loader strict. Existing structured `Fn`, whole-`Dynamic`, or other non-`Macro` schemas are still rejected, including when another definition in the same Snapshot was migrated from a direct quote.
- Add focused regression coverage for one-pass migration, canonical reload, optional/rest preservation, empty capabilities, and mixed modern/legacy input.
- Reproduce against `calcit-lang/calcit.algebra@b15895d05cf9bdf5dcf7686f17799d9ee20b3937`: the current build migrates 2 namespace quotes and 15 definition quotes, writes `algebra.test/in-rust:` as a strict macro contract, then passes `config show` and an idempotent second format.
- Document the reachable split path: structured legacy macro schemas use final-compatible 0.13.51, while earlier direct-quote macros use the current isolated formatter and require human contract refinement afterward.

- 在隔离的 `calcit edit format` loader 中，仅对从旧 direct quote 解码出来的 definition 恢复严格 macro contract：required、optional、rest 参数形状保留为 `Syntax`，expansion 使用 `Expr<Dynamic>`，capabilities 为空。
- 普通 Snapshot loader 继续保持严格。已有结构化 `Fn`、whole-`Dynamic` 或其他非 `Macro` schema 仍会被拒绝，即便同一 Snapshot 中还有另一个刚从 direct quote 迁移的 definition。
- 增加定向回归，覆盖一次迁移、canonical reload、optional/rest 保留、空 capabilities，以及现代/旧输入混合时不放宽校验。
- 在 `calcit-lang/calcit.algebra@b15895d05cf9bdf5dcf7686f17799d9ee20b3937` 上真实复现：当前构建迁移 2 个 namespace quote 和 15 个 definition quote，将 `algebra.test/in-rust:` 写成严格 macro contract，随后 `config show` 成功且第二次 format 幂等。
- 文档明确可达的分流路径：已结构化的旧 macro schema 使用最终兼容 0.13.51；更早的 direct-quote macro 使用当前隔离 formatter，之后必须人工收窄 contract。
