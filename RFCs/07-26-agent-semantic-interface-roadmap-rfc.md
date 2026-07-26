# RFC: 面向 LLM Agent 的语义接口与约束闭环路线图

状态：Draft
日期：2026-07-26
关联：`07-06-semantic-tree-navigation-rfc.md`、`07-19-doc-knowledge-index-rfc.md`、`07-19-type-introspection-consistency-rfc.md`、`05-12-program-diff-rfc.md`、`02-14-project-modernization-roadmap.md`

## 1. 概要

Calcit 已具备一组很适合 LLM Agent 的基础设计：

- 源代码以结构化 Cirru 数据存储，而不是只能按文本行处理；
- definition 级别同时保存 `doc`、`schema`、`examples`、`tags` 和 `code`；
- Markdown 使用 frontmatter 表达检索入口和知识图关系；
- CLI 已提供 definition 查询、结构化搜索、AST 编辑、类型覆盖、调用图和结构化 diff；
- preprocess/type checking 能在执行前暴露一部分错误。

当前主要瓶颈不是缺少单项能力，而是这些能力尚未形成一个统一、稳定、机器可消费的 Agent 接口。Agent 往往需要依次执行 `query peek/schema/examples/usages`、`docs search/graph`、`tree show` 等命令，手工拼接上下文；编辑后又需要自己决定验证范围并解析人类文本输出。

本 RFC 建议保留“代码即数据”和 Markdown frontmatter 两个核心方向，优先补齐以下闭环：

```text
snapshot + docs + types + dependency graph
                  ↓
          Semantic Agent Index
                  ↓
    query context / query type / diagnostics
                  ↓
      transactional edit + affected checks
```

目标是降低 Agent 的工具调用次数、输出 token、路径定位失败率和修改重试次数，而不是单纯增加更多提示文本。

## 2. 现状调查

以下数据来自 2026-07-26 对当前仓库与 CLI 的本地检查。

### 2.1 代码元数据基础较好，但模型存在分层差异

Snapshot 中的 definition 已经包含：

```text
doc / examples / tags / code / schema
```

运行时与编译阶段的 `ProgramDefEntry`、`CompiledDef` 也保留了其中大部分信息，但字段集合并不完全一致，例如 snapshot 中的 `tags` 没有在所有后续结构中继续保留。特殊 builtin 的查询又使用独立 metadata fallback。

这意味着 query、docs、runtime introspection 和 codegen 仍可能从不同来源组装 definition 信息。

### 2.2 文档元数据覆盖仍不足以支撑完整语义导航

当前 `docs/` 中共有 53 个 Markdown：

- 52 个包含 `title`；
- 18 个包含 `summary`；
- 17 个包含知识节点 `id`；
- 只有少量文件声明 `code_refs`。

现有文档图缓存中：

- 510 个 core definition；
- 382 个 definition 自带非空 doc；
- 120 个 definition 自带 examples；
- 只有 5 个 definition 通过 `code_refs` 关联到文档节点。

结论是 frontmatter 适合维护人工精选的概念关系，但不适合要求作者手工枚举全部 API definition。

### 2.3 frontmatter 规则与实际校验不完全一致

文档规范把 `title`、`scope`、`kind`、`category` 描述为必要字段，但当前加载校验主要只拒绝未知 `category`。同时搜索 metadata 和知识图 metadata 由两套手写 parser 分别解析，存在字段行为逐渐分叉的风险。

### 2.4 类型覆盖报告曾存在错误的正向信号

实测：

```text
cr calcit/test.cirru analyze check-types --ns app.main

levels: full=27 partial=0 none=5
kinds: fn=5 data=4 other=23
```

调查时，无法识别的 definition 会进入 `DefKind::Other`，并默认标记为 `CoverageLevel::Full`。例如源码 payload 为 `fn (...)`、但 schema 已经是函数签名的 definition，可能不会按函数统计。

这种结果会让 Agent 错误地把“未分析”理解为“类型完整”。在继续扩大类型提示前，应先修正覆盖率指标的可信度。

当前实现进度（2026-07-26）：第一批改动已让 function schema 成为 callable coverage 的优先来源，并将无法识别的 code/data 从 `full` 修正为 `none`；对应正常、未知与显式 schema 路径已有回归测试。

### 2.5 静态类型能力查询仍缺少统一入口

当前能力分散为：

- `query schema`：definition schema；
- `&methods-of` / `&inspect-methods`：运行时值或类型定义的 method；
- `&inspect-type`：运行时展示；
- preprocess/type inference：编译内部使用；
- `analyze check-types/weak-types`：项目级覆盖分析。

但尚无一个纯静态 CLI 能直接回答：

> 某个类型或某个表达式推断为什么类型？有哪些字段、variant、method、trait、签名和实现优先级？

### 2.6 JSON 输出还不是稳定的机器协议

部分命令支持 `--json`，但语义通常是“在人类输出后追加 JSON”。例如 `query def --json` 仍先打印 doc/schema/Cirru，再输出 `JSON:` 段。

这使 Agent 需要从混合文本中截取 JSON，同时还要处理颜色、tips、command explanation 和运行日志。

### 2.7 编辑缺少事务、revision 前置条件和原子保存

当前结构化编辑通常执行：

1. 读取整个 snapshot；
2. 修改内存 AST；
3. 重新序列化整个 snapshot；
4. 直接写回目标文件。

缺少：

- `--dry-run`；
- “文件仍是我刚才查询的版本”的 revision 检查；
- 多操作原子 transaction；
- 临时文件 + rename 的原子保存；
- 编辑后自动检查受影响 definition。

这在多 Agent、并行命令或长链路任务中容易造成 stale path、覆盖新改动或中断时文件损坏。

### 2.8 Agent 指南和路线图也可能发生契约漂移

当前仓库已经出现“路线图标记能力已完成，但实现后来回退或改变”的情况。单靠自然语言状态与更新时间不足以保证 Agent 读取到的内容仍符合当前 CLI。

需要通过版本化 metadata、可执行示例和 CI 验证把文档事实与实现绑定起来。

## 3. 设计原则

### 3.1 保留代码即数据，不为 LLM 退回纯文本模型

结构化 AST 是 Calcit 的优势。Agent 不应主要依赖行号、正则和文本 patch，而应通过 definition、selector、path 和 subtree 操作代码。

本 RFC 不建议优先拆分 `calcit.cirru`。只要语义查询和编辑接口可靠，单文件物理存储并不是 Agent 的首要瓶颈。

### 3.2 渐进披露，而不是一次输出全部上下文

默认返回最小充分信息，并通过稳定 ID/handle 继续展开：

- summary 优先；
- examples 限量；
- usages/dependencies 限深；
- 大 definition 返回 fragments；
- Markdown 返回相关 section，而不是整篇文件；
- 所有列表支持 limit/cursor。

### 3.3 人类输出与机器输出共用数据模型、使用不同 renderer

命令内部先产生结构化结果，再分别渲染：

```text
typed result
  ├── human renderer
  └── JSON renderer
```

禁止从人类字符串反向解析机器数据。

### 3.4 只把高置信信息当作约束

类型覆盖、weak type 和静态推断必须区分：

- 已证明；
- 部分推断；
- 有意动态；
- 未分析；
- 推断失败。

“未知”不能被统计成“完整”。低置信提示不能阻塞正常代码。

### 3.5 修改操作必须可预览、可验证、可防止 stale write

所有修改都应具备：

- precondition；
- semantic diff；
- dry run；
- atomic commit；
- structured result；
- affected checks。

## 4. 统一 Definition Descriptor

建议抽出供 snapshot、program、compiled metadata、query 和 docs index 共用的只读语义描述：

```json
{
  "id": "calcit.core/to-js-data",
  "revision": "opaque-content-hash",
  "kind": "proc",
  "source": {
    "scope": "core",
    "definition": "calcit.core/to-js-data"
  },
  "schema": {},
  "inferred_type": null,
  "doc": "...",
  "examples": [],
  "tags": ["js-ffi"],
  "dependencies": [],
  "usages": [],
  "methods": [],
  "traits": [],
  "diagnostics": []
}
```

要求：

- `id` 是跨 query 的语义身份；
- `revision` 是并发编辑前置条件，不承诺可读；
- builtin 与 source-backed definition 使用同一结果结构；
- 缺失字段显式为 `null` / 空数组，不靠缺省文本猜测；
- runtime value 不作为 metadata 查询的隐式依赖。

## 5. 统一机器输出协议

### 5.1 参数规范

逐步统一为：

```bash
--format human
--format json
```

兼容期可保留 `--json`，但内部映射到 `--format json` 并逐步弃用。

### 5.2 JSON 模式约束

- stdout 只输出一个 JSON value；
- stderr 承载运行日志、版本、调试信息；
- 不输出 ANSI；
- 不输出 command echo、Explanation、tips；
- 顶层包含 `schema_version`、`command`、`revision`、`data`、`diagnostics`；
- 分页结果包含 `next_cursor`；
- 截断结果包含 `truncated: true` 和可继续查询的 handle。

建议 envelope：

```json
{
  "schema_version": 1,
  "command": "query.context",
  "revision": "...",
  "data": {},
  "diagnostics": [],
  "next": []
}
```

### 5.3 CLI capability manifest

增加：

```bash
cr capabilities --format json
```

返回命令、参数 schema、结果 schema、是否只读、是否修改文件、是否幂等。这样 Agent 不必把 100 多个 CLI 子命令的 help 全部加载进上下文。

## 6. 聚合上下文查询

新增：

```bash
cr query context <namespace/definition> \
  --budget 2500 \
  --format json
```

MVP 返回：

- definition summary；
- schema 与 inferred type；
- 代码概要或 chunk fragments；
- 最多 N 个 examples；
- 直接 dependencies/usages；
- 相关文档 section；
- methods/traits；
- 当前 diagnostics；
- definition revision；
- 推荐的下一步查询 handle。

`--budget` 是近似输出 token/字符预算，不需要依赖具体模型 tokenizer。实现可先使用字符数和节点数的稳定估算。

建议支持 profile：

```bash
--profile understand
--profile edit
--profile debug
--profile document
```

profile 只是预设字段与预算分配，不改变底层数据模型。

当前实现进度（2026-07-26）：已提供 `cr query context <ns/def>` MVP。它返回 definition revision、Snapshot doc/tags/schema features、受预算限制的代码与 examples、直接 dependencies、带 Snapshot path 的 usages、静态 method，以及类型覆盖和 weak-type diagnostics；小型代码节点在 JSON 中保留结构树。特殊 builtin（例如 `to-js-data`）使用 curated metadata 进入同一 envelope。profile、文档 section 自动关联和 cursor 仍待后续实现。

## 7. 静态类型能力查询

### 7.1 类型定义查询

新增：

```bash
cr query type :number --format json
cr query type app.schema/Person --format json
```

当前实现进度（2026-07-26）：已提供 human/JSON typed-result MVP，支持 builtin/参数化类型，以及具有明确静态 schema 的 definition；method 按实际静态分派优先级去重并显示 impl 来源，且不会运行项目入口。字段/variant/签名以及无 schema definition 的进一步推断仍按本 RFC 后续阶段推进。

返回：

- canonical type；
- fields 与字段类型；
- enum variants 与 payload；
- methods 与 method schema；
- trait 来源；
- impl precedence；
- constructors；
- 相关 docs/examples。

### 7.2 表达式位置查询

新增：

```bash
cr query type-at app.main/f --path code@3.2 --format json
```

行为：

- 只执行 parse、macro/preprocess 和 type inference 所需步骤；
- 不运行 init function；
- 返回 inferred type、expected type、bindings、可用 methods；
- 失败时返回结构化 diagnostics；
- 支持 semantic selector，数字 path 只作为兼容定位。

当前实现进度（2026-07-26）：已提供 `type-at` human/JSON MVP。它以 definition 的 Snapshot `code@...` 路径定位表达式，返回 inferred/expected type、confidence、typed bindings、method candidates、evidence、definition revision 和结构化 diagnostics；处理过程只加载并预处理静态 metadata，不执行项目入口。命名 `defstruct`/`defenum` 会保留 source-backed type reference，函数 schema 会进入参数与返回上下文，`intentional-js-ffi` 与 unresolved dynamic 继续分开报告。semantic selector 仍待后续实现。

### 7.3 intentional dynamic

复用或扩展 schema feature：

```cirru
:features $ #{} :js-ffi
```

weak type 分析应区分：

```text
intentional-ffi
intentional-macro
unresolved
legacy
unknown
```

有意动态仍可展示，但默认不与缺失类型混成同一严重级别。

### 7.4 静态强化与动态边界原则

类型强化不以消灭所有 `:dynamic` 为目标。大型 global state、JS FFI、宏展开边界、异构数据交换和确实无法在预处理阶段确定的值仍允许动态；工具必须把这些“有意动态”与遗漏 schema、推断失败分开。其余路径优先保留并传播信息：

- 同质 list/set/map 推断元素、键和值类型，异质集合安全退回 dynamic；
- `atom`/ref 保留初始化值类型；
- 命名 struct/enum 在源码阶段保持稳定 type reference，不依赖构造运行时值；
- 函数 schema 注入参数、return、generics、where 和 variadic rest，传给高阶函数时不丢失；
- `hint-fn local schema` 会细化后续词法作用域中的局部函数值，调用与 callback 检查可直接复用完整签名；
- 函数体内的 partial `hint-fn` 按真实形参数量补齐未声明槽位为 dynamic，而不是把缺失 `:args` 误判成零参数；
- callback 参数按逆变、返回值按协变检查；
- schema 必须反映真实运行时输入。像递归 flatten 这种同时接收集合和标量的函数应明确保留 dynamic，避免错误 schema 配合类型谓词折叠后产生不可靠优化。

当前实现进度（2026-07-26）：上述 collection/ref、source-backed named type、function/rest 与 callback variance 已进入预处理和回归测试；FFI/global state 等边界继续允许显式 dynamic，并可由 `weak-types`/`type-at` 审计。

补充实现进度（2026-07-26）：`:any` 已明确为静态顶类型，用于表达“契约接受任意 Calcit 值”；它与表示未知、双向放弃检查的 `:dynamic` 分离。`check-types`/`weak-types` 不再把 `:any` 误报为未解析动态，`query type :any` 可用于确认该类型契约。

## 8. 文档知识索引升级

### 8.1 自动生成 definition 节点

为 snapshot 中每个 definition 自动生成：

```text
calcit://definition/<namespace>/<name>
```

虚拟节点包含 doc/schema/examples/tags，不要求 Markdown 手工声明 `code_refs`。

当前实现进度（2026-07-26）：`query context` 已为每个可查询 definition 返回稳定的 `calcit://definition/<namespace>/<name>` URI，并同时返回已有 frontmatter `code_refs` 对应的文档节点。将全部虚拟 definition 节点写入统一 docs graph/cache、支持 URI 直接读取，仍属于本阶段后续工作。

frontmatter 的职责收敛为：

- 概念入口；
- 教程顺序；
- requires/related/leads_to；
- 将一个人工文档关联到一组精选 definition。

### 8.2 heading 级 section 节点

每个 Markdown heading 自动生成可寻址 section：

```text
<doc-id>#<stable-heading-slug>
```

缓存记录：

- 文件路径；
- heading 层级；
- start/end line；
- 内容 hash；
- summary；
- 近似 token/字符数；
- code block 类型；
- definition references。

这样 `query context` 可以只返回相关章节。

### 8.3 frontmatter schema 收敛

合并现有两套 parser，形成一个版本化结构：

```yaml
schema: calcit-doc/v1
title: "..."
summary: "..."
scope: core
kind: guide
category: run
status: current
applies_to: ">=0.12.52"
```

校验至少覆盖：

- 必填字段；
- enum 字段；
- duplicate id；
- dangling edge；
- scope 与实际目录一致性；
- `code_refs` 可解析；
- current 文档中的命令/示例可执行。

不要求所有旧文档一次迁移；兼容读取与严格 CI 校验可分阶段启用。

### 8.4 current / draft / historical 隔离

Agent 默认搜索：

```text
current guide + current reference + definition metadata
```

RFC 和 editing-history 只有显式指定时进入结果：

```bash
cr docs search ... --scope rfc
cr docs search ... --scope history
```

避免历史讨论中的旧语法参与默认答案。

## 9. 统一结构化诊断

在现有 `CalcitErr` / `LocatedWarning` 基础上统一为 `CalcitDiagnostic`：

```json
{
  "code": "E_METHOD_NOT_FOUND",
  "phase": "preprocess",
  "severity": "error",
  "message": "...",
  "location": {
    "definition": "app.main/f",
    "path": "@3.2",
    "selector": "path ...",
    "fingerprint": "...",
    "context": "..."
  },
  "expected": ["number method"],
  "actual": ".unknown",
  "related": [],
  "examples": ["calcit.core/&methods-of#example-0"],
  "fixes": []
}
```

要求：

- `code` 稳定，不把可变自然语言编码进 ID；
- `phase` 至少区分 parse、snapshot、macroexpand、preprocess、type-check、codegen、runtime；
- location 同时提供临时数字 path 与较稳定 selector/fingerprint；
- expected/actual 使用结构化值，而不是只拼进 message；
- fix 必须是可预览的结构化 edit，不直接静默应用；
- `.calcit-error.cirru` 与 JSON CLI 使用同一诊断数据，不维护两套协议。

建议新增：

```bash
cr --check-only --format json
cr query diagnostics --format json
```

当前实现进度（2026-07-26）：`analyze check-types/weak-types --format json` 已返回 versioned envelope、scope revision、稳定的 definition ID、类型/intent 分类和 Snapshot path；`query context` 将这些 weak-type 结果映射为带 code/phase/severity/path/intent 的初版 diagnostics。统一 `CalcitDiagnostic` 与 check-only/runtime error 的接入仍待后续完成。

## 10. 事务化结构编辑

### 10.1 单操作参数

为 `cr edit` / `cr tree` 修改命令统一增加：

```bash
--dry-run
--expect-revision <hash>
--format json
--check-after
```

### 10.2 batch transaction

新增：

```bash
cr edit transaction --file changes.cirru
```

transaction 数据示例：

```cirru
[]
  {}
    :op :tree-replace
    :target |app.main/f
    :selector |path ...
    :expect-revision |...
    :code $ quote ...
  {}
    :op :add-import
    :namespace |app.main
    :code $ quote ...
```

执行流程：

1. 读取 snapshot 并计算 revision；
2. 验证所有 precondition；
3. 在内存副本应用全部操作；
4. 校验 snapshot/schema/Cirru；
5. preprocess 受影响 definition；
6. 生成 semantic diff；
7. dry-run 到此结束；
8. 写入同目录临时文件；
9. flush 后原子 rename；
10. 返回新 revision、changes 和 diagnostics。

任一步失败都不修改原文件。

### 10.3 稳定定位

修改目标优先使用：

```text
definition ID + semantic selector + expected subtree fingerprint
```

数字 path 仍保留，但明确为当前 revision 内有效。若 path 指向的内容与 fingerprint 不一致，拒绝修改并返回新候选。

## 11. 受影响范围验证

结合现有 usages、call graph、program diff 和 schema dependency，增加：

```bash
cr verify --changed --format json
```

MVP 行为：

- 读取 Git diff 或最近 transaction result；
- 找到直接修改 definition；
- 计算受影响调用者；
- 执行 snapshot parse/schema/preprocess；
- 推荐而非自动扩大到相关 JS/IR/WASM 测试；
- 返回已执行、未执行和推荐执行项。

不要一开始尝试自动选择所有测试；先保证影响图和建议输出可信。

## 12. MCP / LSP 适配层

不建议先实现庞大的 MCP 或 LSP server。应先稳定 CLI 的 typed result 与 JSON schema，然后增加薄适配层。

### 12.1 可选 MCP 映射

Resources：

```text
calcit://definition/app.main/f
calcit://docs/core/run/query#quick-recipes
calcit://diagnostics/current
```

Tools：

```text
query_context
query_type
query_usages
preview_edit
apply_transaction
verify_changed
```

每个工具声明 input/output schema，以及 read-only、destructive、idempotent 等属性。

### 12.2 可复用的 LSP 语义

即使不立即实现 LSP，也可复用这些成熟概念：

- workspace/document symbols；
- definition/references；
- hover/signature；
- versioned diagnostics；
- code actions；
- versioned workspace edits。

Calcit 的 definition ID 和 AST selector 比纯文本 line/column 更适合作为内部定位，LSP 只作为外部映射。

## 13. Agent 效率基准

Agent 友好性不能只凭主观体验判断。建议建立一组可重复任务：

1. 静态查询 `:number` 支持的 method；
2. 查找 definition、schema、examples 和相关文档；
3. 在包含多个相同 leaf 的定义里修改指定表达式；
4. 新增 definition 并补 import；
5. 诊断并修复 schema arity mismatch；
6. 诊断 method not found；
7. 修改 JS FFI 边界且保留 intentional dynamic；
8. 根据 changed definitions 选择验证范围；
9. 并发 revision 变化时拒绝 stale edit；
10. 从错误诊断定位到最小可运行 example。

记录指标：

- 成功率；
- 工具调用次数；
- CLI 输出字符/token；
- 无效命令次数；
- 定位或语法重试次数；
- 修改后回归数量；
- 完成时间。

每个 Agent API 改动都应用同一任务集做前后对比。

当前实现进度（2026-07-26）：`yarn check-agent-interface` 已建立第一批真实进程 smoke/benchmark，覆盖静态 method 查询、表达式级 type evidence、特殊 FFI builtin context、项目 definition context、类型覆盖和 intentional dynamic 分析。测试要求 stdout 可被直接解析为单个 JSON，并记录每个场景的耗时与输出字节数；该检查已纳入 `yarn check-all`。其余编辑、诊断修复和 stale revision 场景继续按上述清单扩展。

## 14. 分阶段实施

### Phase 0：修正可信度问题

- [x] 修复 `check-types` 中 `Other => Full`；
- [x] 在 `weak-types` 与 `query context` 中区分 `intentional-js-ffi` 与 `unresolved` dynamic；
- [ ] 为现有 query/analyze 结果抽取 typed result（`query type/context`、`analyze check-types/weak-types` 已使用 versioned envelope，其余命令待迁移）；
- [x] 建立第一批 Agent interface smoke/benchmark（6 个只读语义查询/分析场景，后续扩展编辑与诊断任务）。

验收：报告不再把未分析内容统计为完整；基准可重复运行。

### Phase 1：机器协议与静态发现

- 统一 `--format human|json`（`query type/context`、`analyze check-types/weak-types` 已完成）；
- 保证 JSON stdout 纯净（上述四个命令已完成，命令提示走 stderr）；
- 实现 `cr capabilities`；
- [MVP] 实现 `cr query type`（human/JSON、builtin/显式 schema method 查询已完成）；
- [MVP] 实现 `cr query type-at`（expression path、类型/绑定证据、methods、revision、diagnostics 已完成）；
- [MVP] 实现 `cr query context`（bounded metadata/code、revision、dependencies/usages、diagnostics 已完成）。

验收：Agent 能在一次 context 查询与一次可选展开内获得修改一个 definition 所需信息。

### Phase 2：文档与诊断闭环

- 自动生成 definition 文档节点；
- heading 级索引；
- 合并 frontmatter parser；
- 接入应用与 module snapshot；
- 统一 `CalcitDiagnostic`；
- `check-only --format json`。

验收：任一 core definition 都有可寻址节点；高频错误包含稳定 code、phase、location、expected/actual。

### Phase 3：安全编辑与增量验证

- dry-run/revision precondition；
- atomic snapshot save；
- edit transaction；
- semantic diff result；
- `verify --changed`；
- 可选 MCP adapter。

验收：stale edit 不会覆盖新内容；失败 transaction 不修改 snapshot；成功结果可直接驱动下一步验证。

### Phase 4：可选物理存储演进

只有当 benchmark 显示单 snapshot 在 Git 冲突、加载或并发编辑上成为主要瓶颈时，再评估：

- namespace 分片；
- definition 分片；
- source manifest + derived runtime snapshot；
- 常驻增量 semantic server。

这不是前置条件。

## 15. 建议优先拆出的开发任务

1. `fix(analyze): do not classify unknown definitions as full type coverage`
2. `refactor(cli): introduce shared structured command result envelope`
3. `feat(query): add pure JSON format to peek/def/schema/examples`
4. `feat(query): add static type descriptor command`
5. `feat(query): add bounded definition context command`
6. `refactor(docs): unify frontmatter parser and validator`
7. `feat(docs): generate virtual definition and heading nodes`
8. `refactor(diagnostics): unify error and warning structured payloads`
9. `feat(edit): add dry-run and revision preconditions`
10. `fix(snapshot): save edits atomically`
11. `feat(edit): add multi-operation transaction`
12. `test(agent): add agent-interface benchmark scenarios`

## 16. 非目标

- 不要求让 LLM 直接读取完整 `calcit.cirru`；
- 不要求用向量数据库替代当前文本/JSON cache；
- 不要求一次性为所有文档人工补齐 frontmatter；
- 不要求立刻实现完整 LSP；
- 不要求所有动态代码都强制静态化；
- 不要求自动应用 diagnostics fix；
- 不要求立即改变 snapshot 物理存储布局。

## 17. 风险与应对

### 风险：统一 descriptor 变成新的超大耦合结构

应对：descriptor 是只读视图，由较小 provider 组合；runtime 不依赖 query 层。

### 风险：context 命令输出再次无限膨胀

应对：强制 budget、limit、depth、cursor，并把展开项返回为 handle。

### 风险：frontmatter schema 过重，增加作者负担

应对：definition 节点和 section 节点自动生成；人工只维护概念关系和精选入口。

### 风险：type-at 为了查询而隐式运行程序

应对：明确限制在 parse/preprocess/inference；遇到必须执行的宏或 FFI 时返回 unknown/diagnostic，不做静默 runtime fallback。

### 风险：transaction 一次包含过多修改，失败难以定位

应对：每个 operation 都有独立 ID 和结果；失败返回已验证到的步骤，但整体不写文件。

### 风险：机器协议过早冻结

应对：顶层携带 `schema_version`；先稳定最小字段，再增加可选字段。

## 18. 外部设计参考

- SWE-agent, *Agent-Computer Interfaces Enable Automated Software Engineering*: Agent 使用效果与专门设计的代码导航、编辑和执行接口直接相关。
  <https://arxiv.org/abs/2405.15793>
- Language Server Protocol 3.17：可参考 capability negotiation、symbols、references、versioned diagnostics 和 workspace edit。
  <https://github.com/Microsoft/language-server-protocol/blob/gh-pages/_specifications/lsp/3.17/specification.md>
- SARIF 2.1：可参考稳定 rule ID、location/context region、related location 和 fixes；Calcit 不需要照搬完整格式。
  <https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html>
- Model Context Protocol Tools：可参考 input/output schema、structured content 和 resource link；应作为稳定 CLI 语义层之上的薄适配。
  <https://modelcontextprotocol.io/specification/draft/server/tools>

## 19. 最终建议

Calcit 现有设计无需推倒重来。下一阶段最值得投入的不是继续增加彼此独立的查询或提示，而是建设一个统一的语义 Agent 接口：

```text
可信类型信息
+ 自动生成的代码/文档关系
+ 有预算的上下文查询
+ 结构化诊断
+ 带 revision 的事务编辑
+ 受影响范围验证
```

这五项形成闭环后，代码即数据、frontmatter 文档和类型约束才会真正转化为 Agent 的效率优势。
