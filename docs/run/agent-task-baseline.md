---
title: "Calcit Agent 任务基线"
summary: "真实 Agent 任务的轻量验收，以及按需使用的历史工具契约与实验参考"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "agent task baseline"
  - "agent evaluation"
entry_for:
  - "calcit query"
  - "calcit fix"
id: core/run/agent-task-baseline
parent: core/run
related:
  - core/run/query
  - core/run/fix
requires:
  - core/agent
---

# Calcit Agent 任务基线

本文提供真实 Agent 任务的轻量验收方式，并保留 [#1302](https://github.com/calcit-lang/calcit/issues/1302) 的历史任务目录作为按需参考。按 2026-09-26 收敛后的 [#1306](https://github.com/calcit-lang/calcit/issues/1306)，当前 0.23 验收围绕一次实用的 Respo 修改，不要求先执行全部历史任务、冻结模型或重复三次实验。

## 当前任务验收

在独立分支完成一个小的 typed helper 或组件数据修改，实际使用 [#1307](https://github.com/calcit-lang/calcit/issues/1307) 的推断改善。沿用现有 query → edit/fix → check/:tests → JS 运行路径，不新增命令、报告格式或统计 analyzer。

- 记录实际仓库 revision、明确的工具版本、任务目标、关键命令、重试原因和待人工判断点。候选编译器未发布时明确标记 unreleased revision；依赖仍优先使用精确发布版本。
- 提供修改前后源码和 PR，说明减少了哪些重复标注或必要工具步骤；保持严格类型和普通方法调用，不通过扩大 Dynamic、插入未经证明的 unsafe-coerce 或改用 native call 消除失败。
- 语义断言放在 definition `:tests`，验证相关 JS 构建、运行和浏览器目标 smoke。复用已有模块/宿主边界证据，不要求重新迁移整个应用。
- 在同一个任务中核对适用的查询契约、方法信息和局部 JS FFI 行为；可选参数迁移使用独立有界案例，不强塞进应用。遇到具体阻塞先记录复现和负责 issue，不把失败记录为已通过。
- 候选验收后发布，再用公开包复测必要场景。发布后复测不作为发布前的循环依赖；milestone 收尾仍需完成发布和成果 Discussion。

任务的确定性测试只能证明对应行为；没有可比较的模型运行记录时，**不报告效率提升倍数**。以下历史起点与实验口径仅用于主动开展对照研究，不是日常开发或当前发版的额外门禁。

## 历史实验：固定输入与执行边界（按需）

- 普通任务的固定 Calcit 起点为 `538934af13a86e53c6c31592d7ad33ab7da12c45`（0.20.0 加当时的多入口 `fix` 修复）；使用独立、干净的 worktree，不在当前用户 checkout 上重置或覆盖。此 SHA 属于历史堆叠分支，不是 main 的祖先；#1346 的完整改动随后 squash 合入 main `497c1dfd60ac56d5da754ce46447ebd3b535f106`。旧基线仍从固定 SHA 运行；以 main 做后续对照时，必须另记实际起点与两者差异，不把合并 SHA 静默替换为旧起点。
- 真实 JS 消费者起点另行固定：Cumulo Reel `b0b464d21d781042053a589a19d5e8c7f5a09383`，Timegrass `cd2ddb05f88f51267fade6baa3fda681159440bc`。依赖采用这些 revision 自身的锁文件；安装策略、安全年龄门禁、Node 与 Calcit 版本必须写入每次运行记录。
- T06–T08 是已经发生的历史故障，保留各自原始 Calcit 起点，不把今天的候选修复冒充旧基线。任务规范允许阅读 `AGENTS.md`、`calcit docs agents --contract`、目标仓库版本化文档和该任务点名的源码/测试；不预先泄露修复 PR 的 diff 给 Agent。T10–T11 是 held-out 验收组，不用来决定 #1303–#1305 的命名或指南内容。
- 每个任务从固定起点的临时 worktree 开始。修改 Snapshot 前遵守当前 mutation contract；优先 Cirru EDN，只有外部 JSON-only 接口才显式请求 JSON。保存原始指令、命令及 stdout/stderr、最终 diff、测试输出和人工审阅意见。失败不能靠放宽类型、扩大 `Dynamic`、把方法换成 native call、插入未经证明的 `unsafe-coerce` 或修改预期来消除。

## 历史任务目录（按需）

下表的“契约探针”是今天在 `C` 上可重复运行的最低确定性检查；`H` 只标明旧故障的输入起点，旧 commit 未必含修复后的测试文件。真实任务完成还必须审阅目标 diff 与业务行为；探针通过本身不算任务完成。`C` 表示上述普通 Calcit 起点，`R` 表示真实消费者起点，`H` 表示另列的历史起点。

| ID | 输入与任务 | 可观察的完成条件 | 契约探针与失败分类 |
| --- | --- | --- | --- |
| T01 · C | 从 `calcit/test.cirru` 查询 `Number` 的 `.ceil` 方法，找出可调用参数与结果，不翻内部 ABI。 | 查询结果给出方法契约，Agent 用方法调用完成一个带 `:tests` 的数值示例。 | `yarn check-agent-interface` 的 `static type methods` 场景；区分“查询缺契约”“错误调用形态”“测试失败”。 |
| T02 · C | 在 `tests/fixtures/fix-command.cirru` 中，为已有 helper 的未定 schema 查找可证明的具体类型，只改选中的定义。 | `synthesize-schema-v1` 的 preview 指出精确 slot；不能从单个 example 猜公共参数或把嵌套洞变成整个 `Dynamic`。 | `cargo test --test fix_cli schema_synthesis_applies_exact_compiler_evidence_and_is_target_stable`；区分“推断不足”“误应用”“类型回退”。 |
| T03 · C | 把 fixture 中旧 `%::` / `%{}` 名义构造写法迁移为直接 Enum/Struct 构造。 | 保持字段求值顺序、quoted data 和定义级测试；预览、revision 保护的 apply 与重复预览一致。 | `cargo test --test fix_cli surface_latest_preset_migrates_named_enum_and_struct_constructors`；区分“解析错误”“不安全重写”“不幂等”。 |
| T04 · R | 在 Timegrass 的 dayjs/Respo 路径上，为一个明确选择的 JS host 边界补 `Fn :features :js-ffi` 并复用 typed adapter，不授予整个入口信任。 | 参数、返回值和其他 schema 字段保留；严格检查与实际 JS 编译/运行通过，失败边界仍报错。 | `cargo test --test edit_cli schema_feature_edit_preserves_contract_and_works_in_guarded_transaction`，然后在消费者运行 `yarn check-client` 与 JS smoke；区分“feature 放错层”“宿主值未验证”“业务运行失败”。对应 #1285。 |
| T05 · C | 在 fixture 中重命名被普通代码、attached tests、examples 与 schema 引用的 definition。 | resolver 证明的引用和声明原子更新；quoted/macro 不可定位来源拒绝而不部分写入。 | `cargo test --test fix_cli semantic_rename_updates_attached_tests_and_examples_atomically`；区分“漏引用”“错误目标”“部分事务”。 |
| T06 · H | 从 `517006687f1e36126de5f9aff19fa722939af915` 给 `examples/wasi-command/calcit.cirru` 指定不存在的多层 JS 输出目录和普通文件路径。 | 真正写出 `.mjs` 才能报告成功；不可写目标返回失败。 | `cargo test --test js_artifact_output_cli`；历史旧版误报成功，见 #1302 的 T06 评论与 #1311。 |
| T07 · H | 从 `1eae5f702793369dbab880874fd6f9b1f3058243` 在多入口 Cumulo 副本上运行全项目 `fix`；另外复测 #1346 的 browser/Node target 范围。 | 依赖从各入口正确加载；无 target 的整项目预览可扫描 Node-only 定义且只读；显式 browser 局部范围仍拒绝 Node-only 方法。 | `cargo test --test fix_cli whole_project_fix_loads_modules_from_all_entries_and_reports_missing_dependencies` 与 `whole_project_fix_previews_node_only_definitions_without_weakening_entry_checks`；区分“漏依赖”“目标误判”“真正缺失依赖”。对应 #1315。 |
| T08 · H | 从 `88fa280b73469b0cda0e6a5fd25488612f2cca06` 生成同一 namespace 的值引用与名义解码引用。 | JS 只有一个 namespace binding，Node 语法/运行通过。 | `cargo test --test js_namespace_import_cli`；区分“重复导入”“缺少导入”“运行失败”。历史证据见 #1323/#1326。 |
| T09 · R | 从 Timegrass `app.comp.overview/comp-title` 的旧 `? on-click` 参数出发，迁移定义与静态调用点。 | omission、显式 nil、false 与函数值语义逐项证明；无法证明的调用保持 review，绝不猜默认值。严格 Calcit `:tests`、JS 业务运行通过。 | `cargo test --test fix_cli optional_parameter_rule_reports_review_evidence_without_writing`，消费者 `yarn check-client`；区分“缺调用”“nil/缺省混淆”“求值次数变化”。对应 #1286。 |
| T10 · C · held-out | 在 `tests/edit_cli.rs` 的 `prepare_minimal_snapshot` 生成的 Snapshot 上做 guarded `edit transaction`，同时故意让预览的 Snapshot revision 过期。 | 事务整体拒绝且 Snapshot 未变；新 revision 下两项一起成功，人工可审阅 diff。 | `cargo test --test edit_cli schema_feature_edit_preserves_contract_and_works_in_guarded_transaction` 与 `yarn check-agent-interface` 的 `staged edit transaction`；区分“部分写入”“过期 revision”“传输格式混淆”。 |
| T11 · C · held-out | 用 `cirru parse-edn --file` 读取大于参数长度限制的 Cirru EDN，并比较 stdin 与文件结果。 | 源数据的 EDN 结构和值一致；缺输入、文件与 inline 同传均有明确错误。当前命令的 JSON 兼容输出不代表其他查询应默认 JSON。 | `cargo test --test cirru_parse_edn_cli`；区分“截断”“错误输入选择”“格式误判”。 |

## 对照实验的记录与比较规则（按需）

只有主动开展对照实验时，才使用以下完整记录口径：任务 ID、输入仓库和完整 commit、依赖锁文件 checksum、Calcit/Node/Yarn 版本、模型标识与配置、允许的文档和上下文上限、开始/结束时间、每次工具调用及状态/输出字节数、重试和失败分类、最终 diff、行为测试、人工审阅修改及其原因。失败也保留原始记录，不只统计成功样本。对查询、文档或迁移改动，前后使用相同模型、配置、上下文上限和任务起点；代表任务至少各运行 3 次，报告每次结果及波动，不以 3 次样本宣称统计显著。普通应用验收使用上面的轻量记录，不承担这些实验要求。

2026-09-24 的确定性工具基线：在 Calcit `538934af13a86e53c6c31592d7ad33ab7da12c45` 上执行 `cargo test --test fix_cli --test edit_cli --test cirru_parse_edn_cli --test js_artifact_output_cli --test js_namespace_import_cli --quiet`，五个 suite 分别通过 33、10、3、3、1 项（共 50 项）。同一 commit 的 `yarn check-agent-interface` 通过 32/32 场景。测试数量和绿灯只标记现有工具契约，并非 11 个任务各自的 Agent 完成率、耗时或人工审阅成本。

历史记录包括 T06–T08 的确定性前后故障（见 #1302 评论）；T07 的后续 target 误判随 #1346 合入 main，附有最小回归和 Cumulo 原项目只读预览证据。这些记录不构成冻结模型配置的基线或 matched rerun，不能据此宣称模型效率提升。当前任务与消费者验收以收敛后的 #1306、#1285、#1286 为准；缺少历史模型实验不是它们的前置阻塞。
