# RFC: Type Slot 作为显式编译上下文

状态：Partially Implemented（Revision 3）

初版日期：2025-07-13

修订日期：2026-07-30

---

## 1. 摘要

Type slot 是 Calcit 为避免在 UI 框架、dispatch 回调等调用链中大量传递泛型而采用的工程折中：库声明一个类型位置，应用为一次编译选择具体的 enum、struct 或 record 类型。

这个方向仍有价值，但当前 `with-type-slot` 把“编译期类型环境”伪装成“带运行时 body 的函数调用”，产生了两个已经在真实项目中暴露的问题：

1. 单 body 会在预处理阶段擦除，多 body 会残留运行时调用；额外包一层 `do` 才能改变生成结果。
2. slot 绑定保存在 thread-local 栈中，而 definition 编译缓存只按 `ns/def` 复用；所谓局部作用域实际上受首次懒编译顺序影响。

本 RFC 把决策拆成两层：

- **已落地的最小方案**：`with-type-slot` 在预处理后无条件擦除；entry 的 `:type-slots` 在任何 definition 预处理前安装，作为整次运行/检查/codegen 的默认绑定。
- **继续保留为 Draft 的强化方案**：把当前 registry 重构为显式 `TypeSlotEnv`、将环境指纹纳入 compiled cache，并把 slot identity 升级为声明命名空间限定的身份。

最小方案已经消除“为什么加 `do` 才成功”、`@calcit/procs` 版本耦合以及 entry 函数必须包裹整个调用链的困惑。每次 CLI invocation 只选择一个 entry，加载配置后会清空并重建程序状态，因此当前不要求环境指纹进入缓存；同一进程多环境增量编译、同名 slot 冲突等更复杂场景仍由强化方案处理。

## 2. 决策请求

本 RFC 请求分别确认以下决策，不把它们捆绑为一次大改：

| 决策 | 建议 | 兼容性 | 解决的问题 |
| --- | --- | --- | --- |
| A1. `with-type-slot` 无条件擦除 | 已实现 | 源码兼容 | `do` 差异、JS runtime 缺失、协议版本耦合 |
| A2. codegen 遇到残留 slot form 时硬错误 | 已实现 | 仅影响此前偶然可运行的错误路径 | 防止编译器 invariant 再次泄漏 |
| A3. 增加 AST/Rust 回归测试 | 已实现 | 无破坏 | 固化编译期-only 语义 |
| B1. binding 移到 entry 配置 | 已实现最小版本 | 配置格式新增 | 去掉入口 wrapper 和懒编译顺序依赖 |
| B2. 显式 `TypeSlotEnv` 与缓存指纹 | 暂缓 | Rust 内部 Breaking | 多环境增量编译正确性 |
| B3. slot 使用声明命名空间作为身份 | 暂缓 | Schema/序列化可能 Breaking | 跨库同名冲突 |
| B4. 未绑定 slot 不再静默退化 | 暂缓 | 行为 Breaking | 防止类型检查悄悄失效 |

现有 `with-type-slot` 源码仍可继续使用，但新项目应通过 `calcit config set-type-slot` 维护 entry 配置。实现 entry 配置 **不代表批准** 显式环境对象、缓存指纹或 namespaced identity 的重构预算；这些内容仍需满足第 8.7 节的证据门槛。

## 3. 背景与原始动机

### 3.1 问题场景

Calcit 的静态分析能检查 enum variant、载荷数量和载荷类型。但库定义回调签名时，通常不知道应用将使用哪个具体 enum。

例如 Respo 一类库希望表达 dispatch callback 接收应用的 `Op`：

```cirru
;; library schema
deftype-slot :dispatch-op

:: :fn $ {} (:return :unit)
  :args $ [] '*dispatch-op
```

应用定义自己的操作类型：

```cirru
defenum Op
  :add :string
  :remove :tag
  :clear
```

如果把 `Op` 做成普通泛型，它会出现在 EventHandler、component、renderer、dispatch helper 以及中间数据结构的大量 schema 中。Type slot 的目标是让这类“整个应用统一选择一次”的类型不必逐层出现在源码 API 上。

### 3.2 Type slot 不是什么

Type slot 不是：

- 运行时依赖注入；
- 每次函数调用独立推导的泛型参数；
- 可在同一个编译产物中任意切换的动态类型变量；
- 用来绕过所有跨包泛型的通用机制。

它更接近“编译这个 entry 时采用的一组类型配置”。这一定位决定了它不应产生 runtime ABI，也不应由运行时版本决定能否工作。

## 4. 原实现与故障链

### 4.1 原源码模型

应用目前这样绑定 slot：

```cirru
defn main! () $ with-type-slot (:dispatch-op Op)
  setup!
  render-app!
```

预处理器解析 binding，压入 [`TYPE_SLOT_OVERRIDES`](../src/calcit/type_annotation.rs)，在该栈生效时预处理 body，最后弹出 binding。

### 4.2 单 body 与多 body 分叉

当前 [`preprocess_with_type_slot_block`](../src/runner/preprocess/mod.rs) 的尾部逻辑等价于：

```text
if body.len == 1:
  return body[0]
else:
  return (with-type-slot body...)
```

于是：

```cirru
with-type-slot (:dispatch-op Op)
  do
    setup!
    render-app!
```

会因 body 只有一个 `do` 而完全擦除；没有 `do` 的同义代码却会残留 `with-type-slot`。

这不是 `do` 修复了类型，而是源码形状选择了不同的编译路径。

### 4.3 Runtime 与 JS codegen 泄漏

Rust interpreter 注册了 [`with_type_slot_runtime`](../src/builtins/meta.rs)，依次接收已经求值的 body 并返回最后一个值。[JS codegen](../src/codegen/emit_js.rs) 没有对应的 compile-time 分支，因此残留 form 会落入通用 proc 生成路径，并依赖 `@calcit/procs` 导出同名函数。

这造成三层语义不一致：

| 层 | 当前行为 |
| --- | --- |
| 预处理单 body | 完全擦除 |
| Rust 多 body | 依赖 runtime stub，偶然可运行 |
| JS 多 body | 生成 runtime proc 调用，可能在模块加载或调用时失败 |

即使 CLI 与 `@calcit/procs` 版本完全一致，这个 compile-time-only form 也不应要求 runtime 提供实现。版本不一致会扩大故障面，但不是根因。

### 4.4 “局部作用域”并不完全成立

slot override 是 thread-local 栈；definition 一旦完成预处理，compiled result 按 `ns/def` 缓存。一个依赖在 slot A 下首次编译后，在 slot B 下再次引用时会直接命中缓存，而不会重新应用 B。

因此当前语义更准确地说是：

> `with-type-slot` 影响其 body 触发的、尚未进入缓存的传递依赖。

它不是普通词法作用域。下列因素都可能改变结果：

- definition 是否在进入 block 前已经被预热；
- client/server entry 是否在同一进程、同一 compiled cache 中处理；
- watch reload 清除了哪些 definition；
- 两条调用链是否共享一个首次被特化的 dependency。

现有 [entry 测试](../src/bin/cr_tests/type_fail.rs)分别重新加载 client 与 server，只证明“独立加载时不冲突”，没有证明“同一缓存中两个环境可安全复用”。

### 4.5 其他可解释性问题

- `deftype-slot` 被描述为可选声明，导致 slot 名称缺少可靠的声明身份。
- slot 以短字符串作为全局 key，不同库的 `:dispatch-op` 可能冲突。
- 未绑定 slot 静默视为 `:dynamic`，用户无法区分“有意动态”与“绑定没有覆盖到这里”。
- `push` / `pop` 依赖手动配对；普通错误路径已清理，但 panic 或未来新增早退分支可能污染同线程后续编译。

## 5. 设计目标

本提案必须满足以下目标：

1. **不传播源码泛型**：应用级统一类型不需要出现在每层函数和数据结构的参数列表中。
2. **纯编译期**：type slot 不产生 runtime value、runtime proc 或 JS package ABI。
3. **源码形状无关**：单 body、多 body、显式 `do` 的编译期类型效果一致。
4. **缓存可证明正确**：编译结果必须能说明它在哪个 slot 环境中产生。
5. **entry 可隔离**：client/server 可选择不同具体类型，但不能靠首次编译顺序碰运气。
6. **失败可见**：缺失绑定、冲突绑定和环境复用错误必须给出明确诊断。
7. **分阶段交付**：修复当前 bug 不依赖完整架构重构。

## 6. 非目标

本 RFC 不试图：

- 用 type slot 替代普通函数/数据类型泛型；
- 在一个函数的不同调用点自动生成任意数量的泛型特化；
- 允许运行时改变 slot binding；
- 为同一个 JS module 自动复制并重命名整条依赖图；
- 在阶段 A 改变现有 `*name` schema 语法。

## 7. 提案 A：无条件擦除 `with-type-slot`

### 7.1 规范语义

`with-type-slot (:name TypeExpr) body...` 只在预处理期间存在：

1. 解析并校验 binding；
2. 在 slot override 下预处理全部 body；
3. 恢复之前的 slot 环境；
4. 返回不含 `with-type-slot` 的普通 Calcit IR。

预处理输出规则：

| body 数量 | 输出 |
| --- | --- |
| 0 | arity error：至少需要一个 body expression |
| 1 | 该 expression |
| 2+ | 内部顺序表达式，语义等价于已展开的 `do` / `&let () body...` |

实现应直接构造已经展开的顺序 IR，而不是重新插入源码层 `do` macro，避免 body 被第二次宏展开或预处理。

### 7.2 编译器 invariant

预处理完成后的 AST/IR 中不得出现：

- `CalcitProc::WithTypeSlot`；
- slot binding pair 的运行时求值；
- 指向 `@calcit/procs` 的 `with-type-slot` 调用。

Rust evaluator、JS codegen 或 WASM codegen 如果收到残留 form，应返回内部编译错误，例如：

```text
internal compiler error: with-type-slot escaped preprocessing
```

不能继续生成一个看似合法但依赖特定 runtime 版本的调用。

### 7.3 内部表示

阶段 A 可以保留 `CalcitProc::WithTypeSlot` 作为 parser/preprocessor 入口，以降低改动范围。后续可把它改成专用 syntax 节点，彻底删除 runtime dispatch。

推荐的兼容顺序：

1. 先保证所有正常编译路径都擦除；
2. codegen 对泄漏 form 硬错误；
3. 一个发布周期后删除 `with_type_slot_runtime` 与 builtins dispatch；
4. 最后再决定是否将内部枚举从 Proc 移到 Syntax。

这些内部调整不要求应用改源码。

### 7.4 为什么不只给 JS runtime 补函数

为 `@calcit/procs` 增加 `with_type_slot(...xs) => xs.at(-1)` 只能掩盖泄漏：

- 仍然保留单 body/多 body 两条路径；
- 仍然让纯类型功能进入 runtime 协议；
- 仍然要求 CLI/runtime 同步升级；
- WASM 与后续 backend 也必须重复实现无业务含义的 stub；
- 不能解决 compiled cache 与 slot 环境不匹配。

可以在过渡发布中临时提供 JS stub 以改善旧产物错误信息，但它不属于最终设计。

### 7.5 阶段 A 的原实施边界

阶段 A 作为局部编译器修复已经完成，原定边界只触及：

- `src/runner/preprocess/mod.rs`：把多 body 输出改为普通顺序 IR，并拒绝空 body；
- `src/codegen/emit_js.rs` 及必要的 backend guard：阻止残留 form 进入 runtime；
- 定向测试 fixture / Rust tests：覆盖 AST 擦除、返回值和 JS 输出；
- 静态分析文档：删除需要额外 `do` 的暗示，明确 compile-time-only。

阶段 A 本身没有修改：

- snapshot 的 entry schema；
- `TypeSlot` 的序列化格式；
- compiled cache key；
- 应用源码调用方式；
- 普通泛型或 trait 机制。

后续又独立落地了第 8.2 节的最小 entry 配置，但缓存指纹与 namespaced identity 仍停留在 RFC，不因配置功能存在而自动排期。

## 8. Entry 级配置与显式 `TypeSlotEnv`

本节分为已经实现的 entry 配置信息模型，以及仍待证据支持的内部环境重构。

### 8.1 语义边界

一个 codegen/check invocation 选择一个 entry，也选择一份不可变的 type-slot environment。该环境在任何 definition 预处理前建立，在整个可达调用图中保持一致。

概念上：

```text
compile(snapshot, selected_entry, type_slot_env) -> artifact
```

而不是：

```text
evaluate(with-type-slot body) -> 在遇到 dependency 时临时影响编译
```

这与 type slot 的真实用途一致：应用为一次构建选择统一类型，而不是函数执行到某一行时改变类型。

### 8.2 已实现的配置语法

默认 entry 在 `:entries.default` 中绑定短 slot 名，值必须是完整 definition path 或 `:dynamic`：

```cirru
:entries $ {}
  :default $ {}
    :mode :native
    :init-fn |app.main/main!
    :type-slots $ {}
      :dispatch-op |app.schema/Op
```

命名 entry 使用自己的完整配置，可以选择另一类型：

```cirru
:entries $ {}
  :server $ {}
    :mode :native
    :init-fn |app.server/main!
    :type-slots $ {}
      :dispatch-op |app.schema/ServerOp
```

命名 entry 不继承 `:entries.default.type-slots`。这里使用完整 definition path，而不是在配置解析阶段求值任意 Calcit expression，因此配置可序列化、可查询，也不依赖入口函数的执行顺序。对应命令为：

```bash
calcit config set-type-slot :dispatch-op app.schema/Op
calcit config set-type-slot --entry server :dispatch-op app.schema/ServerOp
calcit config type-slots --entry server
calcit config rm-type-slot --entry server :dispatch-op
```

程序加载模块后验证 type path 存在，再在任何 definition 预处理前安装所选 entry 的绑定。局部 `with-type-slot` 兼容形式仍可覆盖 entry 默认值，但新入口不需要 wrapper。

### 8.3 Slot 身份（暂缓）

当前实现继续使用裸 slot 名，以控制迁移范围。若真实项目出现两个依赖库同名 slot 冲突，内部 identity 再升级为：

```text
TypeSlotId { declaring_ns, name }
```

库内仍可书写简短引用 `'*dispatch-op`；在读取该 namespace 的 schema 时解析为完整 identity。entry 配置绑定完整路径，避免两个依赖库都声明 `:dispatch-op` 时互相覆盖。

跨 namespace 显式引用可保留扩展空间，例如：

```cirru
'*respo.schema/dispatch-op
```

是否公开该语法不影响内部先采用 namespaced identity。

### 8.4 编译上下文（暂缓）

当前 CLI 在一次 invocation 加载一个 entry，并在提取程序时重置 registry；它没有把环境对象逐层传入预处理 API。若要支持同一进程中的多 entry 增量编译，应显式接收不可变环境，而不是读取 registry：

```rust
struct TypeSlotEnv {
  bindings: HashMap<TypeSlotId, Arc<CalcitTypeAnnotation>>,
  fingerprint: TypeSlotEnvId,
}
```

`TypeSlotEnvId` 必须稳定反映所有 slot identity 与 concrete type reference。它用于：

- compiled cache 校验；
- watch reload 失效判断；
- query/debug 输出；
- compiled snapshot 元数据；
- 诊断“该 def 已在另一环境下编译”。

### 8.5 缓存策略（暂缓）

强化方案不做多环境自动特化。采用更保守的规则：

> 一个输出 artifact 只允许一个 `TypeSlotEnvId`。

compiled definition 记录环境 ID。遇到不同环境时：

- 新的独立 build/check invocation：清理或使用另一份 compiled cache；
- 同一 artifact 内：明确拒绝，提示为两个 entry 分别生成产物；
- watch 中 entry binding 改变：使全部依赖 slot 的 compiled definitions 失效；第一版可以安全地全量失效，后续再缩小范围。

这避免在尚未设计稳定符号命名和共享策略前，暗中生成多份 specialized JS definitions。

### 8.6 未绑定行为（当前保持兼容）

当前未绑定 slot 静默退化为 `:dynamic`，很容易让用户误以为检查已经生效。

当前未绑定 slot 仍退化为 `:dynamic`；配置也允许显式写 `:dynamic`。是否升级诊断继续保留以下建议规则：

- 可达 schema 引用了已声明 slot，但 entry 未绑定：默认 hard error；
- 应用确实希望关闭检查时，在 entry 中显式绑定为 `:dynamic`，让意图可见；
- 迁移期可先发结构化 warning，再在下一个 breaking release 升级为 error。

示例诊断：

```text
E_UNBOUND_TYPE_SLOT: `respo.schema/dispatch-op` is required by
respo.schema/EventHandler but is not bound for entry `app.main/main!`.
Bind it in :type-slots or explicitly select :dynamic.
```

### 8.7 阶段 B 的证据门槛

阶段 B 只有在至少满足下列一项时才进入实现评审：

1. 有最小复现证明同一 compiled cache 中，definition 在 slot A 下编译后被 slot B 错误复用；
2. watch reload 改变 binding 后出现可重复的陈旧类型检查或错误 codegen；
3. 一个受支持的正式工作流需要在同一进程连续构建多个不同 slot 环境，并且简单清理缓存不可接受；
4. 两个真实依赖库的同名 slot 已产生冲突，而不是仅有理论可能。

进入实现评审前还必须提供基线数据：受 slot 影响的 definition 数量、全量失效耗时、独立 entry 构建耗时，以及在 Respo 等真实项目上的产物差异。若问题可由更小的缓存失效修复解决，应优先另提小改，不直接启用完整 `TypeSlotEnv` 重构。

## 9. 多 Entry 与共享代码

### 9.1 推荐模型

client 与 server 若使用不同的 `Op`，应作为两个独立 compilation units 生成产物：

```text
client entry + ClientOp -> client artifact
server entry + ServerOp -> server artifact
```

它们可以共享源码和普通缓存输入，但不能共享已经受 type-slot 驱动重写影响的 compiled definition。

### 9.2 为什么不能无成本共享

Type slot 不只影响 warning；它还可能参与 enum tuple 识别和类型导向重写。相同源码 definition 在两个环境下可能生成结构不同的 IR。

若要求一个 artifact 同时包含两种环境，编译器必须选择以下一种复杂策略：

- 复制整条受影响调用图并生成稳定的 specialized symbol；
- 保留统一动态表示，放弃相关静态重写；
- 重新引入显式泛型/类型参数。

本 RFC 不隐式承诺这些能力。第一版选择独立 artifact，行为简单且可验证。

## 10. 迁移方案与当前进度

### 已实现：修复语义泄漏

- 已实施提案 A；
- 保持现有 `with-type-slot` 源码有效；
- 多 body 无需 `do`；
- 增加泄漏 invariant 与后端测试；
- 文档明确 type slot 是 compile-time-only。

应用无需迁移。

### 已实现：引入最小 entry 配置

- 已增加 `:type-slots` 配置；
- `with-type-slot` 继续可用并始终在预处理阶段擦除；
- `calcit config type-slots [--entry name]` 展示 bindings；
- `set-type-slot` / `rm-type-slot` 提供安全的 snapshot 修改入口；
- 未绑定 slot 暂时保持 `:dynamic` 兼容行为；
- `TypeSlotEnvId` 和自动提取 wrapper 的迁移命令尚未实现。

迁移前：

```cirru
defn main! () $ with-type-slot (:dispatch-op Op)
  setup!
  render-app!
```

迁移后：

```cirru
;; calcit config set-type-slot :dispatch-op app.schema/Op
defn main! ()
  setup!
  render-app!
```

### 候选 breaking release（未排期）

- entry 可达的未绑定 slot 升级为 error；
- 删除 runtime stub 和 runtime proc dispatch；
- 视迁移情况决定删除 `with-type-slot` 源码语法，或仅保留为 entry 配置的兼容展开形式；
- slot identity 固定为 namespaced declaration。

## 11. 验证计划

### 11.1 阶段 A 必须覆盖

1. 单 body 预处理结果不含 `WithTypeSlot`。
2. 多 body 预处理结果不含 `WithTypeSlot`，保持求值顺序并返回最后值。
3. 空 body 给出明确 arity error。
4. 显式 `do` 与直接多 body 产生等价行为。
5. JS 输出中不存在 `with_type_slot` / `with-type-slot` runtime reference。
6. Rust once、JS once 与 WASM 内部验证结果一致。
7. 使用不包含 type-slot runtime proc 的旧 `@calcit/procs` 仍可运行新生成产物。
8. codegen 人工收到泄漏 form 时稳定返回 internal compiler error。

### 11.2 Entry 配置必须覆盖

1. client/server 在不同 invocation 中绑定同名 slot 到不同类型，结果独立且确定。
2. 默认与命名 entry 的配置 round-trip 不丢失，旧 snapshot 缺少字段时按空 map 读取。
3. 完整 type path、不存在 definition 与 `:dynamic` 有确定行为。
4. entry 默认 binding 可被兼容的局部 `with-type-slot` 覆盖并在退出后恢复。
5. `--check-only`、Rust 与 JS 对同一 entry 使用同一绑定。
6. config set/list/remove 命令不会破坏 snapshot 其他字段。

### 11.3 强化方案启用前必须覆盖

1. 相同 definition 在不同环境间不会从 compiled cache 静默复用。
2. watch 中修改 entry binding 后正确失效。
3. 两个库声明同名短 slot 时，完整 identity 不冲突。
4. compiled snapshot/query 输出可报告 environment fingerprint。
5. 同一 artifact 尝试混入两种 environment 时明确拒绝并给出拆分 entry 的建议。

### 11.4 仓库验证

实现完成后按仓库流程执行：

```bash
cargo fmt
cargo clippy -- -D warnings
yarn compile
cargo test
yarn check-agent-interface
yarn check-all
```

涉及 CLI 查询、编辑或类型分析时，再使用新构建的 `calcit` 在 Respo 等真实项目回归。应验证 Respo 的 entry 配置能让大量 `d! $ :: ...` 调用共享同一 `Op` 检查，并验证旧的多 body 兼容 form 生成 JS 不再依赖 `do`。

## 12. 备选方案

### 12.1 保持现状，只在文档要求包 `do`

拒绝。它把编译器内部 body-count 分支变成用户必须记忆的语法仪式，也无法解释 Rust/JS 差异。

### 12.2 给所有 runtime 增加 `with-type-slot` stub

拒绝作为最终方案。可以短期兼容旧生成物，但会永久扩大 runtime ABI，并掩盖预处理泄漏。

### 12.3 只把 Proc 改成 Syntax

不充分。专用 syntax 能表达 compile-time-only 意图，但若多 body 仍残留，或编译缓存仍忽略 slot 环境，核心问题依然存在。

### 12.4 恢复全局 `bind-type`

拒绝。它无法支持 client/server 独立选择，也使加载顺序和全局副作用更严重。

### 12.5 全面改用泛型

类型理论上最直接，但违背 type slot 的原始工程目标：一个应用统一选择的 dispatch 类型会沿大量库 API 传播。普通局部关系仍应使用泛型；应用级配置关系保留 type slot。

### 12.6 自动按环境做多版本单态化

暂不采用。它需要调用图复制、symbol mangling、增量缓存和 JS module export 规则，复杂度显著高于当前需求。未来若出现“单 artifact 必须容纳多套 slot”的真实用例，再另立 RFC。

## 13. 风险与缓解

| 风险 | 缓解 |
| --- | --- |
| 阶段 A 构造顺序 IR 时丢失 location | 复用 wrapper/head location，并测试 warning/call stack 位置 |
| 删除 runtime stub 影响旧 compiled snapshot | 延迟一个发布周期删除；加载旧 snapshot 时给明确升级提示 |
| Entry 配置增加项目元数据复杂度 | 只接受完整 slot/type path，不引入可执行配置表达式 |
| watch 中改变配置后需要重建环境 | snapshot reload 会重新提取程序并清理编译状态；若未来保留跨环境缓存，再引入环境指纹 |
| 未绑定升级为 error 破坏旧项目 | 先 warning；支持显式 `:dynamic` opt-out；提供迁移查询 |
| Namespaced identity 改变 schema 序列化 | 旧 `'*name` 按声明 namespace 兼容读取，新写出格式另行确定 |
| 两个 entry 误共享 artifact | 当前一次 CLI invocation 只选择一个 entry；支持同进程多 entry artifact 前必须实现 environment ID |

## 14. 可观测性与 Agent 接口

为避免 type slot 再次成为只能从生成代码猜测的隐藏状态，当前已经提供面向人的只读查询：

```bash
calcit config type-slots
calcit config type-slots --entry server
```

后续若 Agent 接口确有需要，再增加单 JSON 的机器输出，例如：

```json
{
  "entry": "app.main/main!",
  "environment_id": "...",
  "bindings": [
    {
      "slot": "dispatch-op",
      "type": "app.schema/Op",
      "status": "bound"
    }
  ],
  "unbound": []
}
```

机器查询不是最小 entry 配置的前置条件。若实现，stdout 必须保持单个 JSON；计时和人类提示写 stderr，遵循现有 Agent CLI 协议。

## 15. 尚待决定的问题

已决定：所有入口统一存放在 `:entries`，`:default` 是无参数入口；每个 entry 各自保存完整 `:type-slots`，不做隐式继承；配置值只接受完整 definition path 或 `:dynamic`。

仍待决定：

1. `deftype-slot` 是否默认 required？当前保持兼容，并以 entry 显式 `:dynamic` 作为 opt-out。
2. 是否在迁移完成后删除 `with-type-slot` 源码语法？当前保留为局部兼容形式。
3. `TypeSlotEnvId` 写入 compiled snapshot 后，旧 CLI 应拒绝还是忽略未知字段？需要结合 snapshot 兼容策略单独确认。
4. slot identity 是否升级为 namespace-qualified declaration，以及源码是否公开跨 namespace slot 引用语法？
5. slot declaration 是否最终改成 namespace metadata，而不再作为 `deftype-slot` form？

## 16. 推荐结论

A1-A3 与最小 B1 已实现。它们建立了最重要的 invariant：

> Type slot 只属于编译器，不属于 runtime。

建议在 Respo 与 client/server fixture 验证通过后采用这套 entry 配置作为当前公开方案。显式环境对象、缓存指纹、namespaced identity 与未绑定 hard error 暂不捆绑发布；只有出现第 8.7 节的证据再继续推进。

这个分阶段方案保留了 type slot “不大量传递泛型”的价值，同时停止让用户依赖 `do`、运行时版本和懒编译顺序理解它。
