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
  - core/run/workflow-entrypoints
requires:
  - core/agent
leads_to:
  - core/run/upgrade
---

# Compiler-guided Source Fixes

`calcit fix` 是所有“检测并改写可证明问题”的统一入口，不再为单条规则增加 `analyze detect-*` 或新的顶层命令。
入口分工与兼容读取边界见 [Calcit CLI 工作流入口收敛](workflow-entrypoints.md)。

`calcit fix` 把编译器已经能够唯一判断的迁移建议映射回 Snapshot source AST。默认命令只做预览；它会在同目录的
staged Snapshot 上尝试替换并重新编译选定 scope，成功后仍不写入原文件：

```bash
calcit calcit.cirru fix
calcit calcit.cirru fix --ns app.main --def render! --format edn
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
- `rename-definition-v1` 是参数化语义重构规则。它要求 `--ns`、`--def` 与 `--to`，只改写 resolver 已证明指向
  同一项目 definition 的源码引用，并在同一事务中移除旧 `:refer`、重命名声明。裸引用会写成完整 namespace 路径，
  避免新名称被调用点的局部 binding 遮蔽；已有 `:as` 限定名会保留 alias。definition-attached tests 与 examples
  使用同一个 resolver trace，schema 则按 compiler-loaded nominal type reference 与 `:where` trait bound 改写；三者与普通 code、imports、声明在
  同一事务中提交。macro 生成引用、dependency source、quoted data 或缺少 source coordinate 的引用会拒绝整个事务。
- `value-to-zero-arg-fn-v1` 是参数化语义重构规则。它要求 `--ns` 与 `--def`，把精确的 `(def name value)` 改成
  `(defn name () value)`，把原 schema 包成零参数函数返回类型，并把 resolver 已证明的项目源码、attached tests 与 examples
  中的读取改成调用。它会改变求值时机：原值在 definition 初始化时求值一次，新函数则在每次调用时重新求值；因此只允许显式
  选择，不进入任何升级 preset。quoted data、macro 生成引用、dependency source、schema type reference、自引用或缺少 source
  coordinate 的引用会拒绝整个事务。
- `synthesize-schema-v1` 是参数化类型改写规则。它要求 `--ns` 与 `--def`，直接读取正常预处理及自底向上的类型推导结果，
  只填补源码 schema 中已有的 `Dynamic` 洞。零参数函数、可推导返回值和 `Ref<T>` 等候选没有剩余洞时标记为
  `machine-applicable`；局部参数、嵌套泛型等位置仍缺少约束时保留精确路径并标记为 `needs-review`，即使传入 `--apply`
  也不写回。该规则不进入升级 preset，不处理 macro、data/trait/impl 声明，也不通过执行程序猜运行时类型。
- `tag-match-to-match-v1` 与 `required-struct-field-v1` 属于只随 Calcit 0.14.15 发布的 migration bridge，不是当前 fix surface。
  升级旧项目时请固定使用 0.14.15 执行规则、review 输出并验证测试；迁移完成后再切换到 0.14.16 或更新版本。
  当前版本若显式请求这两个 rule，会返回稳定错误和上述版本提示，不会继续携带旧 planner 与分析特例。
  replacement 原样保留 receiver 子树且只出现一次，因此不会复制或重排求值。

## 运行版本化升级 preset

`surface-latest-v1` 是第一版冻结集合，按确定顺序展开为
`removed-data-api-v1`、`named-enum-constructor-v1`、`named-struct-constructor-v1` 和
`redundant-do-v1`，其含义保持不变。当前推荐的 `surface-latest-v2` 在这四条之后增加
`single-expression-do-v1`。执行时会按 source path 从内向外应用结构改写，具名构造器区域内的规则合并为一次 guarded replacement，
避免嵌套改写提前移动 source path。Cirru EDN preview 会在 `:data :filters :preset-id` 返回 preset 名称，并在
`:data :filters :expanded-rule-ids` 返回实际执行的规则，Agent 无需依赖人类日志猜测范围：

```bash
calcit calcit.cirru fix --preset surface-latest-v2 --format edn
calcit calcit.cirru fix --preset surface-latest-v2 \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --preset surface-latest-v2 --format edn
```

`--preset` 与 `--rule` 互斥。apply 必须原样重复 preview 的 `--preset`、`--ns` 和 `--def`；第二次 preview
应返回空建议。未来集合发生变化时应发布新的 preset ID，既有 ID 不应静默改变含义。

## 原子重命名 definition 与静态引用

声明和调用点需要一起变化时，不要先执行 `edit rename` 再文本搜索。使用同一个 `fix` preview/apply 闭环：

```bash
calcit calcit.cirru fix --rule rename-definition-v1 \
  --ns app.core --def old-name --to new-name --format edn
calcit calcit.cirru fix --rule rename-definition-v1 \
  --ns app.core --def old-name --to new-name \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --rule rename-definition-v1 \
  --ns app.core --def old-name --to new-name --format edn
```

preview 的每个 usage suggestion 都包含 resolved target 的 `:origin-chain`、source path、fingerprint 和 quoted AST。
规划阶段会遍历普通 code、definition-attached tests、examples 与 schema，但不会把局部同名 binding 或同名依赖定义
当成 usage。tests 和 examples 通过 synthetic lexical scope 复用 compiler resolver；schema 只改写 loader 已保留并能解析到
目标 definition 的 nominal type reference 或 trait bound。测试名称/tags、example 顺序以及无关 schema 保持不变。包含目标名称的 quoted data
不作为可改写 usage，而是按潜在动态消费边界拒绝事务。staged Snapshot
会重新严格预处理整个项目；解析失败、旧引用残留或目标冲突时，原文件保持不变。重复执行原命令时，如果旧 definition
已不存在且新 definition 存在，会返回空 suggestions，作为幂等完成状态。

macro source、macro expansion、quote/quasiquote、dependency-owned source 以及无法映射回 Snapshot 的引用仍然 fail closed，
并列出 blocker；不得把这些边界降级为文本替换。
只想改声明、并明确自行负责调用点时，仍可直接使用 `calcit edit rename`。

## 把 value definition 重构为零参数函数

需要把延迟计算、环境读取或工厂值从普通 definition 改成显式调用时，使用 resolver 驱动的原子重构，不要先改 `defn` 再文本搜索：

```bash
calcit calcit.cirru fix --rule value-to-zero-arg-fn-v1 \
  --ns app.config --def current-config --format edn
calcit calcit.cirru fix --rule value-to-zero-arg-fn-v1 \
  --ns app.config --def current-config \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --rule value-to-zero-arg-fn-v1 \
  --ns app.config --def current-config --format edn
```

例如普通读取 `current-config` 会变成 `(current-config)`；函数值原来作为 callee 的
`stored-handler event` 会变成 `(stored-handler) event`。目标 definition 的原 schema 会成为新零参数函数的返回类型。
普通 code、attached tests、examples、schema 与 definition replacement 在一个 staged transaction 中验证并提交；重复 preview
应为空。

这条规则有意改变生命周期，不承诺语义等价：副作用、时间、随机数、环境变量、对象身份、分配成本以及原先隐含的缓存都会从
“初始化一次”变成“每次调用”。preview 虽能证明引用归属和改写完整性，不能替用户决定这种变化是否正确。应用前必须审阅目标
value expression 和全部 suggestions；如果目标应继续只计算一次，就保留 `def`，不要运行这条规则。

该规则不会改写 quoted data，也不会猜测 macro 展开、动态名称查找、dependency source、把目标当成类型使用的 schema 或
自引用初始化。命中任一边界时整个事务 fail closed，原文件不写入。由于它是用户选择的语义重构而非版本迁移，当前及未来 preset
都不应隐式包含它。

## 从现有推导事实合成 schema

缺失 schema 时先查询单个 definition，不要直接把整个签名填成 `Dynamic`：

```bash
calcit calcit.cirru fix --rule synthesize-schema-v1 \
  --ns app.model --def initial-count --format edn
calcit calcit.cirru fix --rule synthesize-schema-v1 \
  --ns app.model --def initial-count \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --rule synthesize-schema-v1 \
  --ns app.model --def initial-count --format edn
```

规则复用编译器已经产生的 compiled form、局部类型元数据与 bottom-up inference，不维护另一套递归类型模型。
例如 `defatom *count 0` 可以产生 `Ref<Number>`，无参数且返回数值表达式的函数可以产生 `Fn() -> Number`；已有
`Fn(...)->Dynamic`、`Ref<Dynamic>` 或嵌套容器只替换能够由同形状推导结果证明的洞，声明中已有的 generics、`:where`、
features、参数与函数类别保持不变。

结构化 suggestion 的 `:origin-chain` 会给出 compiled inference 候选以及仍未解决的 slot path。只有
`:applicability :machine-applicable` 且未留下 `Dynamic`/未知 callable 的候选会生成事务 operation。`needs-review`
候选只展示更精确的外层结构，例如 `Option<Dynamic>`，不会把未知 payload 扩大成整个 `Dynamic`，也不会在 `--apply`
时部分写回。无法从静态实现恢复类型时命令明确失败；macro、nominal data、trait 和 impl contract 必须继续显式声明。

实现侧证据可补全 value、zero-argument function、`Ref<T>` 与函数返回洞。带参数函数还会遍历普通项目源码中 resolver
确认的调用点；只有调用形态完整、没有 macro/函数值等动态边界，并且同一参数的所有可推导实参类型完全一致时，才把该类型作为
当前 definition graph 的参数约束。缺失、冲突或无法定位的调用证据会保留 `schema.args.<index>`。namespace 名称中以独立点分段出现
`test`、`tests`、`example` 或 `examples` 的普通源码，以及 definition-attached tests/examples，都只作为样本，不作为公共签名证明；
但候选进入 staged transaction 后，引用目标的 attached tests/examples 仍必须在新 schema 下通过严格预处理，
否则整项拒绝且不写回。native 与 JS entry 使用同一预处理事实并应得到相同候选。

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
calcit calcit.cirru fix --rule redundant-do-v1 --format edn
calcit calcit.cirru fix --ns app.main --def render! --rule redundant-do-v1 --format edn
```

Cirru EDN 报告中的 `:data :suggestions` 是检测结果。若列表非空，逐项核对 `:definition`、`:path`、`:original`、`:replacement`、
`:applicability` 和 `:fingerprint`，确认 `:data :validation :status` 为 `passed`，再原样重复 preview 的 scope selectors，
使用同一份报告中的 Snapshot revision 应用。若列表为空，`:data :validation :status` 应为 `not-needed`；跳过 apply，
直接继续验证：

```bash
calcit calcit.cirru fix --ns app.main --def render! --rule redundant-do-v1 \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --ns app.main --def render! --rule redundant-do-v1 --format edn
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

Cirru EDN stdout 是一个完整 value，顶层包含 `:schema-version`、`:command`、Snapshot `:revision`、`:data`、`:diagnostics`
和 `:next`；`:data` 内包含 `:filters`、`:validation` 与 `:suggestions`。每条 suggestion 携带 Snapshot `:source-file`、stable rule ID、diagnostic code、表层 definition/path、subtree fingerprint、
quoted AST 的 original/replacement 与 applicability。结构化 splice 的 replacement 使用 `{} (:$type |splice) (:value $ [])`，
明确表示多个 sibling，而不是伪装成单个表达式。`validation` 说明 staged scope preprocess 是否执行并通过，以及实际检查的
operation 数量；没有可应用 operation 时状态为 `not-needed`。日志和 command echo 只写 stderr，Agent 不需要解析人类文本或生成的 JS。
只有对接 JSON-only consumer 时才显式使用 `--format json`；它与 Cirru EDN envelope 语义等价。

`:data :filters :expanded-rules` 进一步说明每条实际规则的 `:evidence-source`、`:diagnostic-code`、`:lifecycle` 和
`:source-version-required`。当前规则只从当前诊断或 resolved source AST 派生，`:source-version-required` 固定为 `false`；
稳定 rule ID 的版本号只表示协议行为，不表示待迁移项目的来源版本。

确认预览后再应用：

```bash
calcit calcit.cirru fix --ns app.main --def render! --apply --expect-revision 'md5:...'
```

apply 必须原样重复 preview 使用的 `--ns`、`--def` 以及 `--rule` 或 `--preset` selectors。revision 只证明 Snapshot 没有变化，不能证明
更大 scope 中的其他建议也经过审阅；省略 selectors 会重新规划整个项目，而不是只应用上一次预览的子集。

写入流程复用 `tree replace` 的 `--expect` guard 和现有 transaction：规划完成后即使调用方没有显式传
`--expect-revision`，transaction 也必须绑定规划时捕获的 revision。全部 replacement 先在 staged Snapshot 上执行，重新加载并
以调用方选中的同一 `--entry` 及其 target、modules、type slots 预处理选定 scope，最后才原子替换源文件。revision 过期、节点不匹配、替换重叠、parse/schema/preprocess 失败时均不写入。
重复运行同一规则必须返回空 suggestions，不能再次改写。

为了让旧兼容代码在已经启用严格错误的版本中仍可迁移，初次规划会在隔离的兼容 preprocess 中收集 warning 和类型证据；这一步
只产生候选计划。staged Snapshot 应用 replacement 后会重新启动严格 preprocess，严格错误或新增无关 warning 仍会拒绝 preview/apply，
正常的 check、test、JS/WASM codegen 也不会继承规划模式。因此该机制不是降低类型门禁，而是让自动 migration 能先修复门禁指出的旧语义。

为降低 Agent 误写成本，`--apply` 默认要求干净的 Git worktree。确实需要在已有修改上应用时，先审阅 Cirru EDN preview，
再显式加 `--allow-dirty`；非 Git 环境需要显式加 `--allow-no-vcs`。这两个参数只放宽版本控制前置条件，不跳过 revision、
fingerprint、staged preprocess 或原子写入检查。

`calcit fix` 不会自动加入业务默认值、改变错误处理策略、插入 `unsafe-coerce`、扩大 `Dynamic`，也不会把 compiler-owned
core lowering 回写成表层源码。无法证明等价或无法唯一回溯 source origin 的建议保持只读，交给人类决定。
