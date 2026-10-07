# 退场同实现方法别名 .round?、.write-text、.resolve、.reject

按 #1451 的三道闸门（真实下游用量为零、fix 规则已在稳定版发布、实现已解耦）逐个盘点 core 中仍注册的兼容名。只有四个方法别名全部满足，在 0.29.0 删除：Number `.round?`、FsPath `.write-text`、FfiResponse `.resolve` / `.reject`。

依据：
- 下游：浅克隆 199 个含 `calcit.cirru` 的 GitHub 仓库（149 个 2026 年仍有提交），逐 token 统计后人工复核命中。`.resolve` 的命中是命名空间名 `respo.controller.resolve` 与 Quamolit 自定义 trait；`.write-text` 的命中是 calcit-lang/editor 自定义的 ClipboardHost trait；`.round?` 没有命中。calcit-http 已使用 `.resolve!`。
- fix 规则：`core-integer-predicate-v1` 与 `core-effect-method-v1` 均首次出现在 0.28.0-alpha.1，随 0.28.0 稳定发布。
- 实现：四个别名与首选方法在 core 中指向同一函数（`integer?`、`fs-path:write-text`、`ffi-response:resolve`、`ffi-response:reject`），删除只去掉 trait/impl 条目和 Number 方法表中的一行。

范围：core 的 trait、impl 与 Number 方法表；`fix.rs` 中对应的 MethodAliasRule 与 `MethodReceiverKind::Number`；WASM 文件效果检测只认 `.write-text!`；core 附带测试、WASI 示例快照与 fixture 改用首选写法；架构文档 `typed-ffi-capabilities.cirru`、升级说明、fix 文档、api-roles、CalcitAgent 与 FFI 协议文档同步。函数 `round?` 保留（仍有下游调用）。

诊断：`retired_method_migration` 增加 Number `.round?`，以及按 core 名义类型（`calcit.core/FsPath`、`calcit.core/FfiResponse`）识别的三个方法，严格检查报告 `E_RETIRED_METHOD` 并给出首选写法。用户自定义同名 trait 方法不受影响，因为只匹配 core 定义的结构。

fix 规则：两条规则保留在 `core-api-0.28-v1`，preset 规则数不变；`core-integer-predicate-v1` 只改写函数 `round?`，`core-effect-method-v1` 只保留 FfiTask `.cancel` / `.cancel-with`（calcit-fetch 仍在使用，calcit-wss、calcit-fswatch、calcit.std 文档仍引用）。升级说明要求在 0.28.x CLI 上先迁移。

同时补齐 #1568 的查询/文档对齐：`nil->option` 加入 0.28 冻结范围与基线；`nil->option`、`js-nullish->option`、`parse-float`、`get-env` 的示例改用 `Option :some` / `Result :ok` 等首选构造器。

未退场项与原因见 PR 中的盘点表。

验证：`cargo test`（含新增 `strict_type_fail_retired_alias_methods_report_preferred_spelling`）、`yarn check-all`、`yarn check-agent-interface`、`node scripts/core-api-contract.mjs`。
