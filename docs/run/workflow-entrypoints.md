---
title: "Calcit CLI 工作流入口收敛"
summary: "按检查、分析、改写和显式编辑四类任务选择稳定入口，避免为每条规则增加新命令"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "CLI entrypoints"
  - "verification workflow"
  - "normalization workflow"
entry_for:
  - "calcit --check-only"
  - "calcit analyze"
  - "calcit fix"
  - "calcit edit --input-format"
  - "calcit tree --input-format"
id: core/run/workflow-entrypoints
parent: core/run
related:
  - core/run/fix
  - core/run/library-quality
  - core/run/verification-profiles
  - core/run/edit-tree
requires:
  - core/agent
leads_to:
  - core/run/upgrade
---

# Calcit CLI 工作流入口收敛

Calcit 的 CLI 按用户任务收敛入口，而不是按每条诊断或迁移规则增加命令。文档、脚本和 Agent 默认只需要记住下面四类：

| 用户意图 | 统一入口 | 边界 |
| --- | --- | --- |
| 验证当前 entry 是否符合语言语义 | `calcit calcit.cirru --check-only` | 复用正常加载、严格预处理和目标检查，不写文件 |
| 查询与分析只读事实 | `calcit query ...`、`calcit analyze ...` | query 返回源码与配置事实；analyze 组合已有编译器事实，不建立第二套类型系统 |
| 检测并应用可证明的源码规范化 | `calcit calcit.cirru fix ...` | 默认 preview 即检测；只有 machine-applicable 建议才能原子 apply |
| 执行用户明确指定的结构编辑 | `calcit edit ...`、`calcit tree ...`、`calcit cursor ...` | 修改意图来自用户，不冒充编译器自动修复 |

`calcit test`、`calcit js`、`calcit wasm` 与 `calcit wasi` 是执行或目标生成入口，不再复制一套类型检查策略。它们与
`--check-only` 共享 parser、resolver、严格预处理和 target validation。

## 检测与改写只保留一个闭环

当编译器能够给出唯一 source origin、替换内容和语义证明时，检测直接表现为 `calcit fix` 的只读 preview：

```bash
calcit calcit.cirru fix --preset surface-latest-v2 --format json
calcit calcit.cirru fix --preset surface-latest-v2 \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
```

不要为同一规则再增加 `analyze detect-*`、独立 linter 或专用顶层命令。无法证明自动替换的问题保留为严格诊断或
`calcit analyze` 报告；它不应该为了进入 `fix` 而制造不可靠 replacement。Snapshot canonical serialization 继续由
`calcit edit format` 负责，因为它是表示层写回，不是语义迁移。

## 当前语义规范化与兼容读取

兼容读取只解决“旧数据无法被当前表示读取”的问题，并与普通编译隔离。目前这类一次性入口归 `calcit edit format` 所有。
一旦源码已经解析为当前 Snapshot，后续规范化只依赖当前 parser、resolver、strict checker、target validation 和可写 source AST：

- 不读取或猜测生成该源码的 Calcit 版本；
- 不在 core 中保存框架名、仓库名或某次升级顺序；
- 相同的当前 AST 与配置必须得到相同建议；
- preview、revision/fingerprint guard、staged validation 和原子写回统一复用 `calcit fix`；
- 不能唯一回溯到可写源码、涉及 Dynamic dispatch、macro 生成或业务默认值时，只报告诊断。

`fix --format json` 的 `filters.expanded_rules` 会为实际展开的规则报告：

- `evidence_source`：来自 `current-diagnostic` 或 `resolved-source-ast`；
- `diagnostic_code`：稳定诊断或 fix code；
- `lifecycle`：当前规则使用 `current-semantics`；
- `source_version_required`：当前规范化固定为 `false`。

例如 `removed-data-api-v1` 虽保留稳定 rule ID，但其候选直接来自当前 strict checker 的
`W_REMOVED_DATA_API`，不检查 source release。rule ID 中的 `v1` 版本化的是机器协议和行为，不是来源版本。

## Preset 生命周期

Preset 是固定规则集合的便利别名，不是历史语义数据库。已有 preset 发布后保持成员与顺序稳定；推荐集合变化时增加新 ID，
并在升级窗口结束后从默认文档路径退役旧 preset。底层仍有通用价值的当前诊断或 AST 规范化可以继续存在，但不得依赖旧版本号。

仅用于无法由当前语义恢复的兼容 bridge，应固定在能够读取旧表示的已发布工具链中，并在升级手册记录退出路径；当前工具链不继续携带
其 planner。新增规则必须先回答“为什么不能由当前诊断或当前 AST 表达”，否则不进入 catalog。

## 后续能力放置

- 声明式验证 profile 放在 `calcit analyze` 下，组合既有检查并共享一次项目加载；不增加 `calcit verify` 顶层入口。
- Cirru、JSON AST 等 mutation 输入格式扩展现有 `edit/tree/cursor/fix` 的共享输入契约；不增加新的语法节点顶层工具。
- 包含源码或结构化片段的 human 输出统一为 Markdown-compatible 文档，代码使用带语言标记的 fence；稳定自动化继续读取既有 JSON envelope，不增加 `--markdown` 或新的输出命令。
- 跨 definition 与 usage 的语义重构继续通过 `calcit fix` 的 preview/apply 事务暴露；不建立第二套 mutation 协议。

这套边界允许内部能力继续增强，但让人类和 Agent 的入口数量保持稳定。

## Syntax-node 输入只扩展现有 mutation

需要传入一个 syntax node 时，在现有 `edit`、`tree` 或 `cursor apply` 命令上使用同一个
`--input-format` 选项，不增加新的转换命令：

```bash
calcit edit add-example app.main/render! \
  --input-format cirru --code 'quote $ div ({} (:class-name |card))'

calcit tree replace app.main/render! --path @2.1 \
  --input-format json-ast --code '["span","|ready"]'
```

- `cirru`：输入必须是带 `quote` 的 Cirru EDN；`quote` 继续作为语言层 code/data 边界。
- `json-ast`：输入是序列化 AST，字符串表示 leaf，数组表示 list；因此 JSON 字符串 `"[]"` 与 JSON 空数组
  `[]` 分别表示名为 `[]` 的 leaf 和空 syntax list。
- `auto`：仅为旧脚本保留的内容识别路径；新文档和新自动化不应依赖它。

显式格式的 mutation 会在写入前报告选中的格式、解码后的 `leaf` / `empty-list` / `expression` 节点类型、
canonical JSON AST 与结构化摘要。格式或形状错误同时包含选中格式、预期节点类型和实际节点类型，Agent 不需要
通过试写来判断输入如何被解释。`--expect` 仍是单独的 quoted Cirru guard，不受 replacement 的传输格式影响。
