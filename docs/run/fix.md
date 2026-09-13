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
- `single-expression-do-v1` 解包恰好只有一个 payload 的 `(do expr)`。这时 `do` 在分支、调用参数和 binding value
  中也没有组合多个步骤，不会补充求值、失败或类型语义；`defmacro`、`quote`/`quasiquote` 仍作为宏/数据边界保留。
- `named-enum-constructor-v1` 把能静态解析到项目 `defenum` 的 `%:: Result :ok value` 改为
  `Result :ok value`。
- `named-struct-constructor-v1` 把能静态解析到项目 `defstruct` 的 `%{} Person (:name name)` 改为
  `Person :name name`，并保持字段表达式的原始求值顺序。
- `tag-match-to-match-v1` 与 `required-struct-field-v1` 属于已发布的 0.14.x migration bridge，不是 0.15 的 fix surface。
  升级旧项目时请固定使用 Calcit 0.14.15 执行规则、review 输出并验证测试；迁移完成后再切换到 0.15。
  0.15 若显式请求这两个 rule，会返回稳定错误和上述版本提示，不会继续携带旧 planner 与分析特例。
  replacement 原样保留 receiver 子树且只出现一次，因此不会复制或重排求值。

## 运行版本化升级 preset

`surface-latest-v1` 是第一版冻结集合，按确定顺序展开为
`removed-data-api-v1`、`named-enum-constructor-v1`、`named-struct-constructor-v1` 和
`redundant-do-v1`，其含义保持不变。当前推荐的 `surface-latest-v2` 在这四条之后增加
`single-expression-do-v1`。执行时会按 source path 从内向外应用结构改写，具名构造器区域内的规则合并为一次 guarded replacement，
避免嵌套改写提前移动 source path。preview 的 JSON 会在 `filters.preset_id` 返回 preset 名称，并在
`filters.expanded_rule_ids` 返回实际执行的规则，Agent 无需依赖人类日志猜测范围：

```bash
calcit calcit.cirru fix --preset surface-latest-v2 --format json
calcit calcit.cirru fix --preset surface-latest-v2 \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --preset surface-latest-v2 --format json
```

`--preset` 与 `--rule` 互斥。apply 必须原样重复 preview 的 `--preset`、`--ns` 和 `--def`；第二次 preview
应返回空建议。未来集合发生变化时应发布新的 preset ID，既有 ID 不应静默改变含义。

具名构造器规则只处理原型能静态解析到当前项目 nominal definition 的直接源码。匿名 `%:: _` / `%{} _`、
运行时 prototype、依赖中无法回溯的定义、局部同名遮蔽、`defmacro` 以及 `quote`/`quasiquote` 内的数据都会保留。
Struct 规则还要求旧字段是完整的 `(:tag value)` pair；动态字段集合不做猜测。

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

JSON 报告中的 `suggestions` 是检测结果。若列表非空，逐项核对 `definition`、`path`、`original`、`replacement`、
`applicability` 和 `fingerprint`，确认 `validation.status` 为 `passed`，再原样重复 preview 的 scope selectors，
使用同一份报告中的 Snapshot revision 应用。若列表为空，`validation.status` 应为 `not-needed`；跳过 apply，
直接继续验证：

```bash
calcit calcit.cirru fix --ns app.main --def render! --rule redundant-do-v1 \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --ns app.main --def render! --rule redundant-do-v1 --format json
```

第二次 preview 应返回空 `suggestions`，证明规则幂等。随后运行目标 entry 的 `--check-only` 和行为测试。

`redundant-do-v1` 只 splice 已知 variadic body 的直接 `do` 子节点。多表达式 `do` 在以下位置会保留：

- `if`/`case` 分支、函数调用参数和 binding value 等只接收单表达式的位置；
- `defmacro` body，因为 macro 可能需要显式打包返回的语法树；
- `quote`/`quasiquote` 内作为数据保存的代码。

`single-expression-do-v1` 则处理这些普通可执行位置中的 `(do expr)`，因为它没有第二个步骤；同样跳过
`defmacro` 与 `quote`/`quasiquote`。若一个项目同时需要两种整理，直接使用 `surface-latest-v2`，不要靠文本搜索判断层级。

## 自动改写边界

当前 preset 只收录能证明保持表层语义的一对一规则。以下项目继续由严格诊断定位，不能自动加入 preset：

- 已知 Struct 字段访问后的 `.unwrap`：必须先证明 receiver 与调用链，不能全局删除；
- `?` 参数、`Optional`、裸 `nil` 与 `%{}?`：需要选择 Option、Result、Unit 以及新的调用契约；
- Dynamic 收窄、raw primitive 与 `unsafe-coerce`：需要设计 schema 和 capability 边界；
- `get-or`、`first-or` 等到 `.unwrap-or`：需要 fallback 类型和缺失语义证据，后续按独立规则评估；
- Snapshot schema 与 canonical serialization：继续由 `calcit edit format` 负责；
- `tag-match-to-match-v1` 与 `required-struct-field-v1`：继续固定在已发布的 0.14.15 bridge。

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

apply 必须原样重复 preview 使用的 `--ns`、`--def` 以及 `--rule` 或 `--preset` selectors。revision 只证明 Snapshot 没有变化，不能证明
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
