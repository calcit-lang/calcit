# Map/Set 集合组合命名迁移

关联 #1455。

Map 的 `.mappend` 与 `.merge` 在 core 方法表中均指向 `calcit.core/merge`；Set 的 `.mappend` 与 `.union` 均指向 `calcit.core/union`。两组操作保留 Calcit 不可变集合的返回值语义，Map 后出现的同名 key 覆盖前值，Set 合并后去重。List、String、Fn 的 `.mappend` 则是不同实现或语义，不能按名字批量改写。

新增显式 `core-collection-combine-v1` fix，在所选 definition 的 `:code` 中只处理完整的、有至少一个组合参数的方法调用。自动改写要求具体 Map/Set receiver、旧/新方法均为 proven、解析到同一个 core 实现、参数与返回类型一致，而且源码上下文可追溯。开放 receiver、未知 macro 等仅留人工审查；quoted data 不扫描。规则不进入旧升级 preset，不改 `:tests` 或 `:examples`，这些区域须单独检查。

Calcit definition `:tests` 验证 Map 覆盖顺序、Set 去重、多个参数与空集合；Rust CLI 测试只验证预览、revision 防护、应用、幂等以及排除非 Map/Set 的边界。Agent 查询验证两个新名字与旧名字的 proven 实现和类型契约一致。WASM 测试通过真实方法调用验证 Map 覆盖与 Set 去重，不用 native call 替代。

验证：`calcit.core/merge` 与 `calcit.core/union` 的 4 个 definition tests 通过；`collection_combine_fix` 的 2 个 CLI 测试通过；Agent 查询 41/41 通过；完整 `cargo test --locked -q`、`cargo clippy --locked --all-targets -- -D warnings`、`yarn check-all` 与文档检查通过。`bash scripts/test-wasm.sh` 的新 Map/Set 方法调用及原有 WASM 回归通过。Respo 项目使用本地编译 Calcit 的 `--check-only --strict-types` 通过。与 List `.flat-map` 批次合并后的 rebase 已复验，PR 最新 HEAD 的 CI 与 review 仍待完成。
