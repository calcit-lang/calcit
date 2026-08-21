# Runtime Traits for Calcit

> 状态：已落地的设计基线，更新于 2026-08-02。用户语法与示例以 [`docs/features/traits.md`](../docs/features/traits.md) 为准；本文件记录实现边界与后续工作。

## 目标

Calcit 从动态原型式方法分派演进到 trait 模型，但不把语言变成复杂的全静态类型系统。当前设计追求三点：

1. 能力约束是 nominal 的，能从 impl 来源推导，不依赖“恰好有同名方法”。
2. 普通方法调用仍保持轻量、可组合，旧代码可以逐步迁移。
3. native、JS 的语义一致；WASM 只承担内部 codegen 验证，不新增完整 trait runtime。

## 正交的三个层次

### Trait definition

`deftrait` 产生 trait value，包含方法名与方法类型。运行时每次求值得到新的 nominal identity；克隆保留 identity，重新加载定义会产生新 identity，因此旧 impl 不会自动满足新 trait。

Snapshot schema 暂时只保存 symbol 形式的 trait reference。预处理在 trait value 已求值时使用 nominal metadata；只有尚未求值的 schema placeholder 才按定义结构或 bare name 回退。把 namespace-qualified trait id 持久化到 schema 是后续工作。

### Trait impl

`defimpl ImplName Trait ...` 在第二个参数是具体 trait value 时产生 nominal impl：

- impl origin 是该 trait value；
- 方法集合必须与 trait 声明完全一致；
- 每个方法值必须 callable；
- native 能取得函数签名时检查其与 trait method schema 是否匹配。

`assert-traits`、`:where` 和 `&trait-call` 都只接受这种 impl 作为能力证据。不同 impl 的方法不会被拼成一个虚构实现。

### Inherent method bag

历史写法允许把 tag 传给 `defimpl`。它继续产生 originless method bag，并参与 `.method` 查找，以保证旧项目可运行；但它不是 trait impl，不能满足能力约束。`calcit edit format` 对此给出非阻断迁移告警。

这个边界取代旧的“class/prototype”概念：底层仍复用有序 impl record 作为方法表，但语言层不再把方法存在性当作 trait 身份。

## 分派规则

### `.method`

普通 `.method` 是按方法名查找，适合日常动态调用：

- 用户 struct/enum 附加的 impl：从后向前，last-wins；
- core builtin impl list：从前向后，first-wins，保持内建方法优先级兼容性。

它可以命中 nominal trait impl 或 inherent method bag。相同方法名存在多个候选时，顺序决定结果。

### `&trait-call`

`&trait-call Trait :method receiver ...` 先按 trait nominal identity 选择单个 impl，再从该 impl 取方法。它用于消歧义和表达“调用哪个能力”是契约的一部分。

### `assert-traits` 与 `:where`

- `assert-traits` 在 preprocess 提供 local type hint，在 runtime 查找 origin 与目标 trait 一致的单个完整 impl。
- `:where` 使用同一能力关系做 generic substitution/checking。
- 内建类型使用 `calcit.core` 中带真实 origin 的 impl list；初始化期静态检查有一份很小的 bootstrap capability map，避免求值顺序改变告警结果。

## 内建能力

当前主要 core traits：

| Trait | 方法 | 主要内建实现 |
| --- | --- | --- |
| `Show` | `.show` | nil/bool/tag/symbol/CirruQuote，以及 number/string/list/map/set/fn/record/tuple |
| `Eq` | `.eq?` | 与 `Show` 对齐的 scalar/collection/record/tuple 类别 |
| `Add` | `.add` | number/string/list |
| `Multiply` | `.multiply` | number |
| `Compare` | `.compare` | number/string |
| `Len` | `.len` | list/map/set/string |
| `Mappable` | `.map` | list/map/set，以及 Option/Result 自定义 impl |
| `Countable` | `.count` | list/map/set/string/record/tuple |
| `Contains` | `.contains?` | list/map/set/string/record/tuple |

`calcit.internal` 保留不具名的原始方法包，`calcit.core` 在公开 builtin impl list 中通过 `&impl::new Trait method-bag` 将其提升为 nominal impl。scalar literals 共享只含 `Show`/`Eq` 的 impl list。这样不会制造 `calcit.internal -> calcit.core` 的 JS 模块循环，同时 native 宏预处理能看到完整 trait origin。

## 后端约束

### Native

native 是语义基准，也是宏执行、预处理和签名校验的必经目标。trait runtime identity、impl conformance、`assert-traits` 和 `&trait-call` 都完整执行。

### JavaScript

JS 是主要业务运行目标。trait identity 使用对象身份；builtin 注册使用完整 impl list；impl 方法集合与 callable 校验和 native 对齐。类型签名的主要校验发生在生成 JS 之前的 native preprocess。

### WASM

WASM 是内部验证后端。预处理已消除的 trait metadata 不影响 codegen；若运行路径仍残留 `&impl::new`、struct/enum `impl-traits` 或 `&assert-traits`，codegen 明确失败。这里不实现 JS/native 等价的 runtime trait table，也不允许静默返回 `nil` 掩盖语义缺失。

## 已完成的验收口径

- 两个拥有相同方法名的 trait 不能互相满足 `assert-traits`。
- `&trait-call` 只调用目标 trait 的 impl；只有另一个 trait impl 时必须失败。
- concrete `defimpl` 拒绝 missing/extra/non-callable 方法，并在可用时拒绝签名不匹配。
- list 等 builtin 可以通过 `assert-traits` 和 `&trait-call Countable :count` 在 native/JS 一致工作。
- 方法 introspection 显示 builtin method bag 与 origin-carrying trait impl 的明确分层。
- legacy tag-based method bag 继续支持 `.method`，同时触发迁移告警。

## 后续工作（控制复杂度）

1. 在 snapshot/schema 中持久化 namespace-qualified trait reference，删除 bare-name 静态回退。
2. 评估 trait default methods 与 `requires`；只有 native/JS 能共同给出简单、可推导的规则时才开放语法。
3. 观察 method lookup 成本后再决定是否缓存，不预先引入 vtable/trait-object 层。
4. 保持 capability 粒度小；优先增加能改善真实 generic API 的 trait，避免照搬 Rust/Haskell 的完整层级。

不计划恢复 class/prototype 作为第二套公开多态系统，也不计划为 WASM 单独维护一套功能不完整但表面可运行的 trait runtime。
