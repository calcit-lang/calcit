# Runtime Boundary Refactor Plan

> 目标：在**保留 watch 模式热更新**与**保留 JS codegen 依赖原始代码结构**两项能力的前提下，重构 runtime / preprocess / codegen 边界，降低热路径上的 lookup、clone、thunk 污染与全局状态耦合。

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

## 当前进度

截至 2026-03-16，已经完成的不是“纯设计”，而是一部分边界已经落地：

- `DefId` 已经引入，并建立了 `ns/def -> DefId` 稳定索引；
- `CompiledDef` 已经存在，且开始承载 `preprocessed_code`、`codegen_form`、`deps`、`type_summary` 等编译期信息；
- JS / IR codegen 已经切到读取 compiled layer，而不是直接读取 evaled program；
- runtime 已经有并行的 `DefId -> RuntimeCell` 稠密表，作为旧 `EntryBook` 之外的新快路径；
- `CalcitImport` 已开始携带稳定 `def_id` 缓存，runtime lookup 已优先消费它；
- import/runtime lookup 兼容路径里的旧 `coord -> EntryBook` 残余已经清掉，`CalcitImport.coord` 与相关 runner 参数已删除；
- `RuntimeCell` 的最小状态机已经落下，包含 `Cold | Resolving | Ready | Errored`，且 preprocess 的循环保护已从“先写 `Nil`”切到显式 `Resolving`；
- `yarn check-all` 已经作为当前重构的主验证门槛，并且需要先于 `cargo test` 跑通。

同时也要明确，当前还没有真正完成的部分是：

- 旧的 `PROGRAM_EVALED_DATA_STATE` 与 `lookup_evaled_def*` 兼容路径已经删除，`preprocess` 成功路径也不再直接写 runtime cache；compiled fallback 现在只做“读 compiled value”，不再把 compiled payload 回填成 `RuntimeCell::Ready`；
- 全局 `CompiledDef` 已不再携带 `runtime_value` 这类 runtime payload 字段；普通 preprocess 输出已经完全不构造 runtime payload，普通 compiled `Fn/Macro/Proc/Syntax/LazyValue` 都改为需要时从 `preprocessed_code` 临时 materialize；当前剩余耦合主要集中在 compiled/runtime 仍共享同一套 `Calcit` 值表示，以及 codegen snapshot 在 source-backed defs 缺 compiled metadata 时仍可能退回到 runtime-derived snapshot fallback entry；
- `Calcit::Thunk` 仍存在于公开值模型中，但 runtime 主路径已经不再依赖它作为缓存载体：lazy def 的待求值占位优先放进 `RuntimeCell::Lazy`，`Ready(Thunk)` 已被禁止写入 runtime store，preprocess 与普通 lookup 也不再把 runtime lazy cell 重新包装成公共 thunk；当前剩余问题主要转向 snapshot fallback 与少量兼容语义分支；
- watch 模式已经开始利用 compiled deps 做 def 级 invalidation；当前 CLI incremental reload 不再默认按 package 整片清空，而是从 changed defs / ns 头部变更出发做依赖闭包清理。剩余缺口主要是 state slot 尚未引入，以及 watch/reload 回归覆盖还不够强。

这意味着当前状态更准确的表述是：

**Phase 1 和 Phase 2 已经实装，Phase 3A/3B 已经稳定运行，Phase 3D 的主路径删除也已完成；剩余重点转向 3C 以及 preprocess/runtime 的最终拆边。**

## 目标边界模型

建议把当前系统拆成 4 层。

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

当前 `preprocess` 已经开始产出 compiled metadata，成功路径也已经不再直接写 runtime cache；runtime 也不再在 `Cold` 状态下把 compiled payload 回填成 `RuntimeCell::Ready`。对于 lazy def，当前只会先把 compiled metadata 重新播种成 `RuntimeCell::Lazy`，而不是直接伪装成 ready runtime value。这说明边界已经继续前进一步，但还没有完全完成去同构。最终目标依然是：

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

当前状态：主路径已基本收尾。thunk 仍是公开 `Calcit` 值模型的一部分，但 runtime store 已不再接受 `Ready(Thunk)` 这类形态；lazy def 的未求值占位优先放进 `RuntimeCell::Lazy`，raw fallback 若得到 `Calcit::Thunk(Code)` 也会立刻规范化回 lazy cell。`eval_symbol_from_program` 也不再把 lazy thunk 返回给调用方，preprocess 查值路径同样不再把 runtime lazy cell 重新包装成公共 thunk。当前剩余工作主要不再是 thunk 主路径，而是继续压缩 snapshot fallback compiled entry 的存在范围，清理少量旧命名/提示语残留，并补齐更直接的 watch/reload 回归测试。

#### Phase 3D: 删除旧 EntryBook 热路径依赖

- `evaluate_symbol_from_program` 不再依赖 `coord -> EntryBook` 作为主快路径；
- `PROGRAM_EVALED_DATA_STATE` 从主 runtime store 降级为兼容层或被删除；
- watch/reload 改为直接操作 runtime cell table。

当前状态：主体已完成。`coord -> EntryBook` 主快路径已经删除，`PROGRAM_EVALED_DATA_STATE` 也已删除；watch/reload 也已经开始直接基于 `DefId` 与 compiled deps 做依赖闭包失效。剩余问题不再是“还停留在 package 级清理”，而是要把剩余边界情况和回归测试补齐，并继续把状态保留语义从值缓存里彻底拆出去。

### Phase 4: 引入 `PersistentStateLayer`

- `Ref` 与其他跨 reload 状态转入 state slot
- 定义级值缓存与状态对象彻底脱钩

这一步的前提不是“有了 DefId 就可以做”，而是：

- runtime cell 的 identity 已经稳定；
- reload invalidation 规则已经以 `DefId` 为主；
- 不再把“值缓存是否保留”误当成“状态是否保留”。

### Phase 5: 删除旧耦合路径

- 删除 codegen 对 evaled program 的依赖
- 删除 preprocess 对 runtime 写入 `Nil` 的循环保护策略
- 删除或收缩 `Calcit::Thunk` 的公共职责

进入 Phase 5 的标志应该非常明确，而不是“感觉差不多了”：

- `yarn check-all` 与 `cargo test` 在没有旧 evaled fallback 的情况下仍然稳定通过；
- JS/IR codegen 不再从 runtime 值层偷拿任何结构信息；
- runtime cycle detection 已完全基于 `RuntimeCell` 状态机；
- watch reload 可以基于 compiled deps + stable identity 做解释得通的失效。

## 接下来应该怎么做

如果目标是继续往前推进，同时不让风险失控，下一步不应该直接去碰 state slot，也不需要再把重点放回 Phase 3B；更合理的是继续收尾 Phase 3C，并收缩 snapshot/codegen-only fallback。

具体就是：

1. 继续收缩 runtime-derived snapshot fallback entry 的存在范围，优先区分哪些定义只是 runtime-only 注入，哪些本应来自 source/compiled 数据；source-backed defs 则优先补成真正的 compiled def。
2. 给新的 compiled-deps reload invalidation 补更直接的 watch/reload 回归测试，并继续收缩仍需兜底的边界情况。
3. 仅在确有必要时，再继续清理少数仍保留公开 thunk 语义的兼容分支；不要再把重点放回 runtime 主路径。
4. 每一步都以 `cargo fmt && yarn check-all && cargo test -q` 为门槛，而不是只跑 Rust 单测。

换句话说，下一步的目标不是“再引入一个新层”，也不是回头重复 3B，而是：

**把已经接进去的 runtime state machine 和 snapshot/codegen 边界，从“只剩最后几座桥”继续推进到真正职责清晰、桥接范围可解释。**

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

## 建议的第一步实现

如果只做一个高 ROI 起步动作，建议先做：

**先引入 `DefId + CompiledDef`，并把 JS codegen 从 evaled program 上拆下来。**

理由：

- 这是最容易与现有 runtime 并存的一步；
- 能立刻切断“为了 JS 保留 thunk”这一层耦合；
- 一旦 codegen 不再依赖 runtime value，后续 runtime/state 分层会简单很多。

## 最终判断

在保留 watch 模式与 JS codegen 的前提下，Calcit 仍然可以做激进重构，而且值得做。

真正需要放弃的不是功能，而是这件事：

**“同一个 runtime 值对象同时承载源码、预处理结果、惰性求值状态、热更新身份与 codegen 输入。”**

只要不再坚持这件事，边界就能重新变清晰。
