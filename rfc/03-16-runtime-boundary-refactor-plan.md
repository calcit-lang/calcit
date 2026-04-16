# Runtime Boundary Refactor Plan

> 调整后的目标排序：先把 **compiled/runtime 边界站稳并让结构清晰**，其次争取 **热路径性能收益**，再次才考虑 **减少实体与语义收口**。watch 热更新与 JS codegen 继续保留，但不再作为继续扩张新层的理由。

## 为什么现在做

当前实现把 3 类本应分开的信息混在了一起：

- 源代码与预处理后的代码表示；
- 运行时求值缓存；
- 跨 reload 保留的状态。

这直接导致几个后果：

- `preprocess` 会为初始化写入 `Nil` 占位，再递归求值/包 thunk，再写回全局状态；
- runtime 为了 JS codegen 保留 `Calcit::Thunk(Code)`，code 与 value 共存于一套值表示中；
- hot reload 过去只能依赖旧的 evaled store 做粗粒度清理，难以做精确失效；
- lookup 热点落在全局字符串查表与容器扫描上，而不是落在真正的业务执行上。

这些问题并不是 watch 模式或 JS codegen 本身要求的，而是**当前把两者都编码进 runtime 值模型**造成的。

## 不变约束

本重构默认以下语义保持不变：

1. watch 模式下，`namespace/def` 对应的状态在定义未发生实质性变化时可保留。
2. JS codegen 必须能获得足够稳定的代码表示，不能依赖“值已经在 Rust runtime 里算完”。
3. thunk / lazy top-level def 的用户语义保持一致。
4. 现有宏、preprocess、trait/method 校验规则在外部行为上不主动改变。
5. 调试能力允许暂时退化，后续由专门工具补足。

## 核心判断

要保留这两项能力，同时让边界变清晰，必须接受一件事：

**compile-time representation 和 runtime representation 不能再共用同一个 `Calcit` 值对象。**

也就是说：

- codegen 看的是编译产物；
- runtime 看的是求值状态；
- 持久状态看的是稳定 identity 绑定的 state slot；
- 三者不再复用一份“既像代码又像值”的对象。

这不是语言语义变化，而是实现层面的去同构。

## 调整后的判断

这份方案到现在仍然有意义，但它的意义已经变化：

- 这不再是一份适合继续扩张的“四层重构蓝图”；
- 更准确的定位，是一份 **compiled/runtime 拆边收官计划**；
- 继续推进的目标，是把已经落地的边界收紧、补齐回归测试、删除迁移期桥接；
- 暂时不继续推进的目标，是引入更多长期实体来追求理论完备。

换句话说，当前阶段要继续的是“收尾”，不是“扩编”。

## 当前唯一目标

当前不再追求把文档里的四层模型一比一做完，真正执行时只守下面 4 条边界：

1. `preprocess == compile`：只负责确保 `CompiledDef` 存在，不再承担“顺手返回一个可运行值”的通用职责。
2. `run == runtime`：只负责从 `RuntimeCell`/compiled executable payload 取运行值，不再替 metadata/codegen 补结构。
3. `reload == invalidation`：只负责 source change apply 与 `DefId` 依赖闭包失效，不再维护额外 package 级兼容语义。
4. `codegen == compiled snapshot`：只读取 compiled snapshot；任何 runtime fallback 都视为过渡期兼容，而不是长期设计。

这 4 条如果站稳，后续性能优化才会变得局部且可解释：

- preprocess 的性能只看编译与依赖收集；
- run 的性能只看 `DefId` 索引、runtime lock 和求值；
- reload 的性能只看变更种子与反向依赖图；
- codegen 的性能只看 compiled snapshot 构造与 emitter。

## 当前不做

为了保证进度，这一阶段明确不做：

- 不引入新的长期层级实体来“补完”理论模型；
- 不把 `PersistentStateLayer` / `StateSlotId` 推进成当前实现目标；
- 不再扩张新的 compiled/runtime 双向桥接 helper；
- 不为了照顾少量旧路径，继续保留“preprocess 顺手执行、runtime 顺手提供 codegen 结构”这种混合语义。

## 当前收官清单

- `[已基本完成]` 把 `preprocess` 公开入口收敛为 `ensure_ns_def_compiled()`，并把只需要 ensure 的调用点迁走。
- `[进行中]` 继续缩小 runtime-derived snapshot fallback，直到它只剩明确、可解释的过渡语义，或者可以被完全删除。
- `[待继续]` 补齐 watch/reload 回归测试，特别是 changed defs、ns header、removed defs、依赖闭包 invalidation。
- `[待继续]` 清掉迁移期命名和重复 lookup helper，让 `program` 成为唯一边界聚合点。

## 当前进度

截至 2026-03-17，已经完成的不是“纯设计”，而是一部分边界已经落地：

- `DefId` 已经引入，并建立了 `ns/def -> DefId` 稳定索引；
- `CompiledDef` 已经存在，且开始承载 `preprocessed_code`、`codegen_form`、`deps`、`type_summary` 等编译期信息；
- JS / IR codegen 已经切到读取 compiled layer，而不是直接读取 evaled program；
- runtime 已经有并行的 `DefId -> RuntimeCell` 稠密表，作为旧 `EntryBook` 之外的新快路径；
- `CalcitImport` 已开始携带稳定 `def_id` 缓存，runtime lookup 已优先消费它；
- import/runtime lookup 兼容路径里的旧 `coord -> EntryBook` 残余已经清掉，`CalcitImport.coord` 与相关 runner 参数已删除；
- `RuntimeCell` 的最小状态机已经落下，包含 `Cold | Resolving | Ready | Errored`，且 preprocess 的循环保护已从“先写 `Nil`”切到显式 `Resolving`；
- JS codegen 现在会显式跳过 core 中仅由 runtime 提供的 placeholder 定义，以及 `syntax`/`proc` 这类本就不应按普通顶层值发射的定义；这也移除了 `calcit.core.mjs` 中形如 `eval = &runtime-inplementation` 的伪导出；
- `clone_compiled_program_snapshot()` 已开始按“仅收集缺口定义”的两阶段方式补齐 snapshot，而不是先整表 clone source/index/runtime 全局状态后再筛选；
- runtime-derived snapshot fallback 现在进一步收窄为“仅对被现有 compiled graph 实际引用到的 runtime-only defs 生效”；只要 source-backed def 仍存在，或 runtime-only def 只是未被引用的残留 runtime 值，就不会再因为旧 runtime 值而静默补出 snapshot entry；
- runtime-only snapshot fallback 也不再原样嵌入任意 `RuntimeCell::Ready` 值；当前会先把 runtime 值转成可快照的 Calcit 数据/代码形式，像 `ref` / `buffer` / `any-ref` / 运行时函数句柄这类本就不稳定的 runtime-only 值将直接被跳过；
- runtime-only snapshot fallback 已不再携带 source/schema/doc/examples 这类 source 元数据，snapshot 填充任务本身也不再为 fallback 路径 clone 整个 `ProgramDefEntry`；
- snapshot 补缺现在也只会在真正拿到 compiled def / runtime-only fallback 时才创建 namespace entry；source-backed rebuild 失败不再留下空壳 compiled file；
- `seed_runtime_lazy_from_compiled()`、`lookup_compiled_runtime_value()`、`lookup_codegen_type_hint()` 已开始按需读取 compiled 字段，而不是在热路径上先 clone 整份 `CompiledDef`；
- `runner`/`lib`/`preprocess` 主调用方已经迁到“先取 compiled executable payload，再按需求值”的边界；旧的 `lookup_compiled_runtime_value()` 兼容包装已删除。
- IR/codegen type-hint 查询已不再通过执行 compiled payload 来补信息；metadata 查询现在只依赖 compiled/source schema 与现成 runtime 值。
- runtime symbol lookup 已不再假设 compiled 执行会回填 runtime cache；执行后的二次 runtime reread 兼容分支已移除。
- `preprocess` 读取已编译定义时，lazy def 现在优先经由 `RuntimeCell::Lazy` 求值，不再绕过 runtime 状态机直接执行 compiled payload。
- `run_program_with_docs` 已切到 `ensure_ns_def_compiled() + evaluate_symbol_from_program()`，不再依赖 `preprocess` 返回入口值。
- `runner` 内部两处“runtime cell -> compiled executable fallback”逻辑已合并到统一 helper，减少了边界复制和不一致分支。
- `preprocess` 的宽松读取路径也已并到同一组 helper，不再单独复制一份 `RuntimeCell::Lazy`/compiled fallback 分支。
- 默认 scope 下的 thunk 求值入口也已收敛到 `CalcitThunk::evaluated_default()`，减少 runtime/lazy 入口上的样板逻辑。
- runtime cell 与 compiled executable fallback 的统一入口现已下沉到 `program` 层；`runner` 只保留 runtime state 到用户态错误的映射。
- `evaluate_compiled_def()` 现已退回 `program` 内部私有 helper；compiled payload 执行不再作为跨模块公共入口暴露。
- `preprocess` 已不再依赖 lenient runtime/compiled probe 作为前置判断；预处理阶段现在只检查 runtime cell / compiled metadata，读取已编译定义值则走专用 helper，不再复用通用 lookup 包装。
- compiled output 写入接口已改为 payload struct 传参，`program` 内部构造链不再依赖 11 参数长调用；现有 clippy `too_many_arguments` 噪音已被顺手收掉。
- `resolve_runtime_or_compiled_def()` 现在只负责调度；runtime cell 求值与 compiled payload 执行已拆成独立私有 helper，内部边界更接近最终形态。
- runtime-only 路径中的 `seed + lookup cell + resolve cell` 已继续收敛成单独 helper，`resolve_runtime_or_compiled_def()` 现在更明确地只做 `runtime or compiled` 两段调度。
- compiled execution 热路径已改为直接借用 compiled payload；执行时不再额外 clone 一份 `preprocessed_code`，只有测试/显式查询 executable code 时才保留复制语义。
- 仅剩测试使用的 `lookup_runtime_or_compiled_def_lenient()` 兼容包装也已删除；lenient 语义现在直接通过 `resolve_runtime_or_compiled_def(..., RuntimeResolveMode::Lenient, ...)` 覆盖。
- `clear_runtime_caches_for_reload()` 的兼容入口已改为先生成 package 范围的伪 `ns` 变更，再统一复用 `clear_runtime_caches_for_changes()` 的依赖闭包 invalidation，而不再维护独立的 package 扫描清理逻辑。
- `yarn check-all` 已经作为当前重构的主验证门槛，并且当前门槛重新保持可通过。
- 当前阶段的固定门槛 `cargo fmt && cargo test -q && yarn check-all` 已重新可通过。

同时也要明确，当前还没有真正完成的部分是：

- 旧的 `PROGRAM_EVALED_DATA_STATE` 与 `lookup_evaled_def*` 兼容路径已经删除，`preprocess` 成功路径也不再直接写 runtime cache；compiled fallback 现在只做“读 compiled value”，不再把 compiled payload 回填成 `RuntimeCell::Ready`；
- 全局 `CompiledDef` 已不再携带 `runtime_value` 这类 runtime payload 字段；普通 preprocess 输出已经完全不构造 runtime payload，普通 compiled `Fn/Macro/Proc/Syntax/LazyValue` 都改为需要时从 `preprocessed_code` 临时 materialize；当前剩余耦合主要集中在 runtime 执行路径仍会把 compiled executable payload materialize 成公共 `Calcit` 值，而 metadata / codegen 查询已基本不再走这条路；snapshot fallback 已不再对 source-backed defs 静默兜底，也不再为失败的 source-backed rebuild 保留空壳 namespace，未被 compiled graph 引用的 runtime-only 残留值不会再进入 snapshot，而且即便被引用，像 `ref` / `any-ref` 这类不可稳定序列化的 runtime-only 值也会被跳过。
- `Calcit::Thunk` 仍存在于公开值模型中，但 runtime 主路径已经不再依赖它作为缓存载体：lazy def 的待求值占位优先放进 `RuntimeCell::Lazy`，`Ready(Thunk)` 已被禁止写入 runtime store，preprocess 与普通 lookup 也不再把 runtime lazy cell 重新包装成公共 thunk；当前剩余问题主要转向 snapshot fallback 与少量兼容语义分支；
- watch 模式已经开始利用 compiled deps 做 def 级 invalidation；CLI incremental reload 主路径与 reload 兼容入口现在都已统一复用 changes-based 依赖闭包清理，而不再维护一套独立 package 清理逻辑。剩余缺口主要是 state slot 尚未引入，以及 watch/reload 回归覆盖还可以继续加密。

这意味着当前状态更准确的表述是：

**Phase 1 和 Phase 2 已经实装，Phase 3A/3B 已经稳定运行，Phase 3D 的主路径删除也已完成；剩余重点转向 3C 以及 preprocess/runtime 的最终拆边。**

## 基于最近 samply 的现状补充（2026-03-17）

已使用 `profiling/samply-once.sh` 对 `calcit/test.cirru` 与 `calcit/fibo.cirru` 进行 release 采样，当前热点特征：

- `program::materialize_compiled_executable_payload` 仍在热路径出现，说明 runtime 对 compiled payload 的过渡 materialize 仍有收缩空间；
- `CalcitTypeAnnotation::extract_schema_value/schema_key_matches_any` 频繁命中，说明 hint/schema 解析与匹配仍是可优化热点；
- `CallStackList::extend_owned` 在 fibo 采样中有可见占比，说明运行态栈构造仍可能偏积极；
- 采样中可见 `serde_json` 热点，主要来自 profiling/输出链路，不应直接等同于 steady-state runtime 核心开销。

结论：边界拆分方向正确，但“runtime materialize 收缩 + type annotation 热点收敛”已经进入可以直接动手优化的阶段。

## 目标边界模型

建议保留这套分层模型作为**分析框架**，而不是要求实现层面继续一比一落四层实体。

### 1. Source Layer

输入资产：

- snapshot / changeset
- 原始 Cirru
- 文档、schema、examples

特点：

- 只负责装载、diff、持久化
- 不承担 runtime lookup
- 不承担 codegen 的最终消费格式

### 2. Compiled Layer

新增核心对象：`CompiledDef`

建议字段：

- `def_id`: 稳定定义 ID
- `version_id`: 本次编译版本号
- `kind`: `Fn | Macro | LazyValue | ConstValue | SyntaxProxy | NativeProxy`
- `source_meta`: `ns/def/doc/schema/examples`
- `preprocessed_code`: 预处理后的 lowered form
- `codegen_form`: 供 JS/IR 输出的稳定表示
- `deps`: 定义级依赖列表
- `type_summary`: 参数/返回值/trait 约束摘要
- `state_policy`: `Stateless | PreserveAcrossReload | ResetOnChange`

职责：

- 承接 preprocess 的主要产出
- 为 codegen 提供输入
- 为 runtime 提供可执行 payload

关键变化：

- `emit_js` 不再读取 evaled program，而是直接消费 `CompiledDef.codegen_form`
- thunk 不再承担“保留 code for codegen”的职责

### 3. Runtime Layer

新增核心对象：`RuntimeCell`

建议状态机：

- `Cold`
- `Resolving`
- `Ready(Calcit)`
- `Errored(CalcitErr)`

可选扩展：

- `ReadyLazy(Calcit)` 用于保留部分 lazy 语义

职责：

- 仅管理定义的求值状态
- 只缓存运行时值，不保存 codegen 所需代码
- 通过 `DefId` 直接索引，而不是热路径上反复字符串查找

关键变化：

- 旧的 `PROGRAM_EVALED_DATA_STATE` 已删除，runtime store 现在实际就是按 `DefId` 索引的 runtime cell table
- `evaluate_symbol_from_program` 首先解析到 `DefId`，之后只在 runtime table 中操作
- `Calcit::Thunk` 可逐步降级为 runtime 内部机制，而非通用值构造器

### 4. Persistent State Layer

新增核心对象：`StateSlot`

建议形式：

- `StateSlotId`
- `owner_def_id`
- `generation`
- `payload`

职责：

- 保存需要跨 reload 保留的状态
- 与 runtime 值缓存分离
- 在 hot reload 时按稳定 identity 决定复用或重置

关键变化：

- “保留状态”不再等于“保留整个 def 的 evaled value”
- `Ref` / atom / runtime resource 迁移到显式 state slot 体系

当前判断：这一层保留为后续方向，但**不作为当前阶段的执行目标**。只有在 watch/reload 语义已经被现有 compiled/runtime 边界稳定支撑、且确实遇到状态保留表达不足时，才值得继续引入。

## 两项关键能力如何保留

### A. 保留 watch 模式

现状问题：

- reload 主要通过局部删除 evaled defs 实现；
- 定义与状态耦合，导致失效粒度粗；
- 同一个 def 的代码变更与状态保留难以分开判断。

重构后：

1. source diff 生成 changed defs 集合。
2. 仅重编译受影响的 `CompiledDef`。
3. 根据依赖图传播 invalidation 到 `RuntimeCell`。
4. `PersistentStateLayer` 按 `state_policy + identity` 判断是否复用。
5. `reload-fn` 基于新的 compiled graph 与 preserved state 执行。

语义收益：

- 代码热更新与状态保留不再互相污染；
- 可以做更精确的 def 级失效，而不是 package 级清理；
- watch 路径的运行时开销更可控。

### B. 保留 JS codegen

现状问题：

- `emit_js` 直接从 evaled program 读取定义；
- value defs 必须保留成 thunk 才能拿到 code；
- runtime 值表示被 codegen 需求反向塑形。

重构后：

- codegen 仅读 `CompiledDef.codegen_form`
- runtime 是否已经求值，与 codegen 无关
- lazy value 是否在 Rust runtime 里缓存，也与 JS 输出无关

语义收益：

- JS codegen 与 runtime 生命周期解耦；
- thunk 从“跨层共享对象”退化为“编译/运行时的内部策略”；
- 后续 IR/JS backend 可单独优化。

## 需要引入的稳定 identity

这是整个重构的关键。

建议新增：

- `NsId`
- `DefId`
- `CompiledVersionId`
- `StateSlotId`

规则：

- `DefId` 对应语义上的“这个定义”，跨编译版本保持稳定；
- `CompiledVersionId` 对应某次重编译产物；
- `StateSlotId` 对应可保留状态的拥有者；
- `coord`/字符串查找只能作为 debug 与兼容路径，不再作为主索引。

## 对 thunk 的重定义

当前问题不在于 thunk 本身，而在于 thunk 暴露成了 runtime 公共值模型的一部分。

建议分两步处理：

### 阶段一：保留外部语义，收缩内部职责

- 用户仍可观察到 lazy top-level def 行为
- 但 codegen 不再读取 `Calcit::Thunk`
- `Calcit::Thunk` 只用于 runtime 求值过程

### 阶段二：把 thunk 从通用值中移出

- 仅在 `RuntimeCell` 内部表达 lazy state
- runtime 对外暴露的仍是正常 `Calcit`
- codegen / preprocess 不再依赖 thunk 值分支

这一步会显著降低 evaluator、preprocess、codegen 三方的共享复杂度。

## 对 preprocess 的新边界定义

建议明确：

- preprocess 的职责是把 source form 变成 `CompiledDef.preprocessed_code`
- preprocess 可以读取 `CompiledDef` 依赖与类型摘要
- preprocess 不直接写 runtime value table
- preprocess 不再通过写 `Nil` 到 evaled defs 来打断循环

循环依赖处理改为：

- 编译层使用 `Compiling / Compiled / Failed` 状态；
- runtime 层使用 `Resolving / Ready / Errored` 状态；
- 两套状态机分别处理，不再混用。

这里需要特别澄清一件事：

当前 `preprocess` 已经开始产出 compiled metadata，成功路径也已经不再直接写 runtime cache；预处理前置检查也不再通过 lenient runtime/compiled probe 侧向触发执行路径。runtime 也不再在 `Cold` 状态下把 compiled payload 回填成 `RuntimeCell::Ready`。对于 lazy def，当前只会先把 compiled metadata 重新播种成 `RuntimeCell::Lazy`，而不是直接伪装成 ready runtime value。这说明边界已经继续前进一步，但还没有完全完成去同构。最终目标依然是：

- `preprocess` 负责产出 `CompiledDef`；
- runtime 在真正需要值时，才从 compiled payload 驱动求值；
- 循环检测不再依赖“先写一个 `Nil` 到 runtime 再回来填值”。

## 数据结构建议

本次重构不是简单把 `EntryBook` 换成 `HashMap`，但主路径的数据结构必须同步升级。

建议：

- `DefId -> RuntimeCell` 使用稠密表或 `Vec<RuntimeCell>`
- `ns/def -> DefId` 使用只读索引表（初始化后稳定）
- source/meta 数据仍可保留 `HashMap`
- state slot table 使用稠密索引或 slab 风格容器

原则：

- 字符串查找只发生在装载/编译边界；
- steady-state runtime 尽量只做整数索引；
- 热路径避免 `Arc<str>` 比较和容器扫描。

## 当前执行面

不再把后续工作定义成“继续推进到完整四层”，而是改成下面三个收官面：

### A. 站稳 compiled/runtime 边界

- 继续删除仍停留在迁移期的桥接 helper 与兜底分支；
- 保证 metadata/codegen 查询不再借 runtime 执行补信息；
- 保证 runtime lookup 不再假设 compiled 执行会隐式回填缓存；
- 把 `program` 维持为边界聚合点，避免 `runner`/`preprocess` 再各自复制一份 fallback 逻辑。

### B. 补齐 watch/reload 回归测试

- 直接覆盖 changed def、namespace header 变更、依赖闭包失效；
- 验证 source-backed def 不会被 runtime-derived snapshot 静默复活；
- 验证 lazy/runtime-only def 的 fallback 仍符合当前保留语义；
- 把 `cargo fmt && yarn check-all && cargo test -q` 作为固定门槛。

### C. 做减法而不是加层

- 优先合并过渡期命名、兼容包装、重复 helper；
- 暂不引入 `PersistentStateLayer` / `StateSlotId` 这类新实体；
- 只有在现有模型无法表达真实需求时，才新增一层概念。

## 迁移阶段

### Phase 0: 约束冻结

- 明确哪些行为必须保持
- 给 watch / reload / js codegen 补回归测试
- 记录当前热点和基线
- 验证顺序固定为：`cargo fmt && yarn check-all && cargo test -q`

### Phase 1: 引入 `DefId`，不改外部行为

- 建立 `ns/def -> DefId` 索引
- lookup 仍兼容旧路径
- 为后续 runtime/compiled 分层做准备

当前状态：已完成。

### Phase 2: 引入 `CompiledDef`

- 把 preprocess 产物显式化
- `emit_js` 切到 `CompiledDef.codegen_form`
- 仍保留旧 runtime 值缓存

当前状态：主体已完成，但 codegen snapshot 仍允许从 runtime ready 值做 fallback 填补空洞。

### Phase 3: 引入 `RuntimeCell`

- 将 `PROGRAM_EVALED_DATA_STATE` 重写为 runtime cell table
- thunk 改为 runtime 内部状态机的一部分
- `evaluate_symbol_from_program` 改为 `DefId` 驱动

这里不应该一次性硬切，建议拆成 4 个小阶段：

#### Phase 3A: 并行 runtime 索引

- 建立 `DefId -> runtime slot` 稠密表；
- 先让 runtime slot 成为主快路径；当时 `write_evaled_def` / reload / changeset 仍与旧表并行同步；
- `evaluate_symbol_from_program` 优先走 `DefId`，旧路径保留 fallback。

当前状态：已完成。

#### Phase 3B: 显式 RuntimeCell 状态机

- 把 `Option<Calcit>` 提升为 `RuntimeCell`；
- 最少先实现 `Cold | Resolving | Ready | Errored`；
- 把“循环中先写 `Nil`”替换成显式 `Resolving`。

当前状态：已完成，且正常 runtime 路径已经不再依赖兼容 lookup。`preprocess` 已不再把 `Resolving` 暂时转成 `Calcit::Nil`，而是把“确保已 preprocess”与“读取可用值”分开；并且普通 preprocess 输出已经完全不再构造 runtime payload。compiled fallback 也已经不再把 cold def 写回 `RuntimeCell::Ready`，而只是临时读取 compiled value。全局 `CompiledDef` 已不再保存 `runtime_value`，普通 compiled `Fn/Macro/Proc/Syntax/LazyValue` 改为按需从 `preprocessed_code` materialize。codegen snapshot 现在会优先把 source-backed 缺口补成真正的 compiled def，只有补不上时才退回 runtime-derived snapshot fallback。剩余串线主要是 compiled/runtime 仍共用 `Calcit` 这套值表示，以及 snapshot fallback 仍作为最后兜底存在。

#### Phase 3C: thunk 职责内收

- `Calcit::Thunk` 继续保留外部语义；
- 但 runtime 内部缓存与 lazy 状态优先放进 `RuntimeCell`；
- 减少 thunk 对全局写回和 code 表示的承担。

当前状态：主路径已基本收尾。thunk 仍是公开 `Calcit` 值模型的一部分，但 runtime store 已不再接受 `Ready(Thunk)` 这类形态；lazy def 的未求值占位优先放进 `RuntimeCell::Lazy`，raw fallback 若得到 `Calcit::Thunk(Code)` 也会立刻规范化回 lazy cell。`eval_symbol_from_program` 也不再把 lazy thunk 返回给调用方，preprocess 查值路径同样不再把 runtime lazy cell 重新包装成公共 thunk，且预处理前置检查已不再通过 lenient lookup 间接复用执行路径。JS codegen 还额外收掉了一层旧桥接：core 中由 runtime 提供的 placeholder 定义、以及 syntax/proc 名称，不再伪装成普通 JS 顶层导出。当前剩余工作主要不再是 thunk 主路径，而是继续压缩 snapshot fallback compiled entry 的存在范围，并视需要继续补 watch/reload 回归测试。

#### Phase 3D: 删除旧 EntryBook 热路径依赖

- `evaluate_symbol_from_program` 不再依赖 `coord -> EntryBook` 作为主快路径；
- `PROGRAM_EVALED_DATA_STATE` 从主 runtime store 降级为兼容层或被删除；
- watch/reload 改为直接操作 runtime cell table。

当前状态：主体已完成。`coord -> EntryBook` 主快路径已经删除，`PROGRAM_EVALED_DATA_STATE` 也已删除；watch/reload 主路径与兼容 reload 入口也都已经统一基于 `DefId` 与 compiled deps 做依赖闭包失效。剩余问题不再是“还停留在 package 级清理”，而是要把剩余边界情况和回归测试补齐，并继续把状态保留语义从值缓存里彻底拆出去。

### Phase 4: 引入 `PersistentStateLayer`

- `Ref` 与其他跨 reload 状态转入 state slot
- 定义级值缓存与状态对象彻底脱钩

这一步的前提不是“有了 DefId 就可以做”，而是：

- runtime cell 的 identity 已经稳定；
- reload invalidation 规则已经以 `DefId` 为主；
- 不再把“值缓存是否保留”误当成“状态是否保留”。

当前判断：**暂停**。这一步不是当前瓶颈，也不符合“先让结构更清晰、再减少实体”的目标排序。除非后续出现无法用现有 compiled/runtime 边界解释的 reload state 问题，否则不进入实现。

### Phase 5: 删除旧耦合路径

- 删除 codegen 对 evaled program 的依赖
- 删除 preprocess 对 runtime 写入 `Nil` 的循环保护策略
- 删除或收缩 `Calcit::Thunk` 的公共职责

进入 Phase 5 的标志应该非常明确，而不是“感觉差不多了”：

- `yarn check-all` 与 `cargo test` 在没有旧 evaled fallback 的情况下仍然稳定通过；
- JS/IR codegen 不再从 runtime 值层偷拿任何结构信息；
- runtime cycle detection 已完全基于 `RuntimeCell` 状态机；
- watch reload 可以基于 compiled deps + stable identity 做解释得通的失效。

当前判断：保留为**收尾检查表**，不再视为一个需要继续扩展设计面的阶段。

## 接下来应该怎么做

下一步不再是“补完大设计”，而是按下面顺序收官：

1. 先做热点导向减法：继续收缩 `materialize_compiled_executable_payload` 的热路径触发频率，减少 runtime 侧重复 materialize。
2. 再收敛类型系统热路径：优先减少 `hint/schema` 解析与匹配的重复工作（缓存、去重分支、减少中间分配）。
3. 继续收缩 runtime-derived snapshot fallback，只保留真正 runtime-only defs 需要的兜底；source-backed defs 一律优先走 compiled/source 数据。
4. 补齐 watch/reload 回归测试，重点覆盖 changed defs、ns header、removed defs、依赖闭包 invalidation，以及 snapshot fallback 不误补 source-backed defs。
5. 清理还停留在迁移期的 helper、命名和双份 lookup 分支，让 `program` 成为唯一边界聚合点。

换句话说，下一步的目标是：

**把已经接进去的 runtime state machine 和 snapshot/codegen 边界，推进到职责清晰、测试充足、且不再继续引入新实体。**

## 预期收益

### 性能

- steady-state runtime 从字符串 lookup 转向整数索引
- preprocess 不再反复通过 runtime state 做协调
- codegen 不再污染 runtime 值模型

### 架构

- compile / runtime / state 三层边界清晰
- hot reload 失效规则可解释、可测试
- thunk 语义从“跨层共享”变成“局部实现细节”

### 可维护性

- 性能问题更容易定位到某一层
- 调试工具可针对不同层独立建设
- 以后若加 IR backend，不需要继续借 runtime 值做桥接

## 主要风险

1. 旧代码默认假设“所有定义最终都能在 `Calcit` 值层找到表示”，重构后会打破这个心智模型。
2. reload 语义如果没有清晰 identity 规则，容易出现状态误保留或误清理。
3. 宏与 preprocess 若隐式依赖 runtime 当前行为，需要在阶段 2 前先全面梳理。
4. 调试输出会暂时退化，因为 call stack / source mapping 需要重新挂接到 `CompiledDef`。

## 当前建议的第一步

如果只做一个高 ROI 的下一步动作，建议先做：

**先基于 samply 热点收缩 runtime materialize 触发频率，并配套保留 `yarn check-all` + `cargo test` 的语义门槛。**

理由：

- 这是当前最直接、可量化的性能收益来源；
- 这与“run == runtime”边界固化目标一致，不会引入新实体；
- 在现有测试门槛下可快速验证语义不回退。

## 最终判断

在保留 watch 模式与 JS codegen 的前提下，这条线仍值得继续，但应该以“收官和减法”为主，而不是继续做激进扩层。

真正需要继续放弃的不是功能，而是这件事：

**“同一个 runtime 值对象同时承载源码、预处理结果、惰性求值状态、热更新身份与 codegen 输入。”**

只要不再坚持这件事，并且不再为它继续引入额外层级，边界就能重新变清晰，而且实现会更可控。
