---
title: "Compiler-guided Source Fixes"
summary: "使用 calcit fix 预览并原子应用带 revision、fingerprint 与 quoted AST 的编译器指导型迁移"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "calcit fix"
  - "source migration"
  - "automatic migration"
entry_for:
  - "calcit fix"
id: core/run/fix
parent: core/run
related:
  - core/run/edit-tree
  - core/run/upgrade
requires:
  - core/agent
leads_to:
  - core/run/upgrade
---

# Compiler-guided Source Fixes

`calcit fix` 把编译器已经能够唯一判断的迁移建议映射回 Snapshot source AST。默认命令只做预览；它会在同目录的
staged Snapshot 上尝试替换并重新编译选定 scope，成功后仍不写入原文件：

```bash
calcit calcit.cirru fix
calcit calcit.cirru fix --ns app.main --def render! --format json
```

当前稳定规则包括：

- `removed-data-api-v1` 复用编译器已有的 `W_REMOVED_DATA_API` 解析结果。只有一对一的名称迁移
  （例如 `tuple-enum` 到 `enum-definition`）标为 `machine-applicable`；`tuple?` 需要在值判断与定义判断之间选择，因而只返回
  `requires-review`，其 `replacement` 为 `null`。
- `redundant-do-v1` 整理 `defn`、`fn`、`let` 及嵌套 `do` 的 variadic body。父结构本来就按顺序执行多项、
  并以最后一项作为返回值时，这条规则会把直接子节点的单层 `do` splice 到父 body。`if` 分支、调用参数、binding value
  以及 `defmacro`、`quote`/`quasiquote` 数据不在自动修改范围内；macro 是否把多项 body 打包成一个表达式需要保留显式语义。
- `tag-match-to-match-v1` 与 `required-struct-field-v1` 属于已发布的 0.14.x migration bridge，不是 0.15 的 fix surface。
  升级旧项目时请固定使用 Calcit 0.14.15 执行规则、review 输出并验证测试；迁移完成后再切换到 0.15。
  0.15 若显式请求这两个 rule，会返回稳定错误和上述版本提示，不会继续携带旧 planner 与分析特例。
  replacement 原样保留 receiver 子树且只出现一次，因此不会复制或重排求值。

## 检测并修复冗余 `do`

`defn`、`fn` 和 `let` 的 body 本身支持多个顺序表达式，并以最后一项作为结果。下面的外层 `do` 不补充
类型推断或求值能力，只增加 AST 层级：

```cirru.no-check
defn render! (state)
  do
    println |rendering
    view state
```

`redundant-do-v1` 会把它安全整理为：

```cirru.no-check
defn render! (state)
  println |rendering
  view state
```

先用 preview 检测，不要直接 apply：

```bash
calcit calcit.cirru fix --rule redundant-do-v1 --format json
calcit calcit.cirru fix --ns app.main --def render! --rule redundant-do-v1 --format json
```

JSON 报告中的 `suggestions` 是检测结果。逐项核对 `definition`、`path`、`original`、`replacement`、
`applicability` 和 `fingerprint`，并确认 `validation.status` 为 `passed`。然后原样重复 preview 的 scope selectors，
使用同一份报告中的 Snapshot revision 应用：

```bash
calcit calcit.cirru fix --ns app.main --def render! --rule redundant-do-v1 \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --ns app.main --def render! --rule redundant-do-v1 --format json
```

第二次 preview 应返回空 `suggestions`，证明规则幂等。随后运行目标 entry 的 `--check-only` 和行为测试。

规则只 splice 已知 variadic body 的直接 `do` 子节点。以下位置会保留：

- `if`/`case` 分支、函数调用参数和 binding value 等只接收单表达式的位置；
- `defmacro` body，因为 macro 可能需要显式打包返回的语法树；
- `quote`/`quasiquote` 内作为数据保存的代码。

因此“检测冗余 `do`”应使用 fix preview，而不是正则搜索，也不会作为普通 compiler warning 混入类型诊断。

JSON stdout 是一个完整 value，包含 `schema_version`、`command`、Snapshot `revision`、filters、validation、suggestions、diagnostics
和 `next`。每条 suggestion 携带 Snapshot `source_file`、stable rule ID、diagnostic code、表层 definition/path、subtree fingerprint、
quoted AST 的 original/replacement 与 applicability。结构化 splice 的 replacement 使用 `{"$type":"splice","value":[...]}`，
明确表示多个 sibling，而不是伪装成单个表达式。`validation` 说明 staged scope preprocess 是否执行并通过，以及实际检查的
operation 数量；没有可应用 operation 时状态为 `not-needed`。日志和 command echo 只写 stderr，Agent 不需要解析人类文本或生成的 JS。

确认预览后再应用：

```bash
calcit calcit.cirru fix --ns app.main --def render! --apply --expect-revision 'md5:...'
```

apply 必须原样重复 preview 使用的 `--ns`、`--def` 和 `--rule` selectors。revision 只证明 Snapshot 没有变化，不能证明
更大 scope 中的其他建议也经过审阅；省略 selectors 会重新规划整个项目，而不是只应用上一次预览的子集。

写入流程复用 `tree replace` 的 `--expect` guard 和现有 transaction：规划完成后即使调用方没有显式传
`--expect-revision`，transaction 也必须绑定规划时捕获的 revision。全部 replacement 先在 staged Snapshot 上执行，重新加载并
预处理选定 scope，最后才原子替换源文件。revision 过期、节点不匹配、替换重叠、parse/schema/preprocess 失败时均不写入。
重复运行同一规则必须返回空 suggestions，不能再次改写。

为了让旧兼容代码在已经启用严格错误的版本中仍可迁移，初次规划会在隔离的兼容 preprocess 中收集 warning 和类型证据；这一步
只产生候选计划。staged Snapshot 应用 replacement 后会重新启动严格 preprocess，严格错误或新增无关 warning 仍会拒绝 preview/apply，
正常的 check、test、JS/WASM codegen 也不会继承规划模式。因此该机制不是降低类型门禁，而是让自动 migration 能先修复门禁指出的旧语义。

为降低 Agent 误写成本，`--apply` 默认要求干净的 Git worktree。确实需要在已有修改上应用时，先审阅 JSON preview，
再显式加 `--allow-dirty`；非 Git 环境需要显式加 `--allow-no-vcs`。这两个参数只放宽版本控制前置条件，不跳过 revision、
fingerprint、staged preprocess 或原子写入检查。

`calcit fix` 不会自动加入业务默认值、改变错误处理策略、插入 `unsafe-coerce`、扩大 `Dynamic`，也不会把 compiler-owned
core lowering 回写成表层源码。无法证明等价或无法唯一回溯 source origin 的建议保持只读，交给人类决定。
