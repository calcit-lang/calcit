# RFC：Calcit 静态类型系统长期演进路线

状态：Draft

日期：2026-08-21

## 摘要

Calcit 长期继续借鉴 Rust 与 MoonBit：使用名义数据类型、Enum、Option/Result、trait、穷尽模式
匹配、局部推断、明确 unsafe/FFI 边界和工具链一体化，逐步提高“typed core 中静态通过即不会发生
普通类型错误”的可信度。

这里的“借鉴”不是复制语言表面。Calcit 保留持久化数据、GC/运行时值模型、Snapshot、热更新和
多后端特点；不因为 Rust 成功就引入与当前 value model 不相容的 borrow checker，也不因为 JS
生态复杂就复制 TypeScript 的结构类型、union 与 overload。

路线优先级是：先堵住精度静默丢失和 FFI unsafe 边界，再完善控制流与局部推断，然后根据真实
抽象需求扩展 trait。每一步必须通过生态 quality baseline 与真实消费者回归渐进落地。

## 当前基础

Calcit 已有：

- primitive、List/Map/Set/Ref/Fn 等参数化 schema；
- 名义 Struct、Enum、Trait、Impl 及定义值类型；
- 显式 generics、TypeVar、`:where` trait constraints；
- `Option<T>`、`Result<T,E>`、`Unit`、`JsNullish<T>`；
- applied generic Struct/Enum；
- bottom-up 推断和调用点泛型绑定；
- `check-types`、`weak-types`、`quality` 和结构化诊断；
- JS/native/WASM 等后端前的统一预处理。

这些足以支持一条渐进增强路线，不需要重做类型 AST 或切换到完全不同的语言家族。

## 需要解决的核心矛盾

### `Dynamic` 同时表示意图和失败

当前 `Dynamic` 既可以表示维护者明确选择的开放边界，也可能来自：

- 缺少容器参数；
- 无法绑定的 TypeVar；
- 未解析 type slot；
- 旧 Snapshot 或未知 schema；
- 推断无法继续时的兼容 fallback。

后续 `weak-types` 虽能找回部分证据，但类型检查阶段已经把“允许任意值”和“我们还不知道”混在
一起。长期应分离 intentional Dynamic 与内部 Unknown/Unresolved。

### 静态正确与后端正确尚未完全闭环

纯 Calcit、JS FFI、native dylib 和内部 WASM 验证后端承担的保证不同。逻辑 schema、target
capability、ABI transport 和真实宿主值不能被一个 `matches` 结果混成同一层。

长期模型应是：

```text
source schema
  -> inference/type checking
  -> capability/target validation
  -> backend contract/ABI validation
  -> runtime tests for external facts
```

每层只增加证据，不让 runtime fallback 反过来伪装成静态推断成功。

### 大型框架缺少低摩擦的类型表面

Editor/Respo 类项目中的 Dynamic 集中在 store、component props、dispatch、effect callback 和
生命周期。这既有历史债务，也说明当前显式 schema 在高阶 UI 代码中成本偏高。解决方案优先是
改进框架公共类型、局部推断和诊断，而不是直接放宽为结构类型。

## 借鉴范围

### 从 Rust 借鉴

- Enum/Struct 作为主要领域数据模型；
- Option/Result/Unit/Never 的明确控制流含义；
- match 穷尽性和不可达分支诊断；
- trait/impl coherence 与明确的泛型约束；
- unsafe 是小而可审计的边界；
- 错误带 expected/actual、来源和可执行修复路径。

暂不借鉴所有权/借用语义。Calcit 的 persistent collection、runtime object、热更新和多后端模型
与 Rust 资源生命周期不同。若未来需要文件句柄、socket、WASM linear resource 等线性能力，
应先以独立 resource/effect RFC 证明需求，不把 borrow checker 作为类型系统成熟度指标。

### 从 MoonBit 借鉴

- 局部、可预测的类型推断，减少重复 annotation；
- 名义 ADT、pattern matching、trait 与泛型的统一开发体验；
- 编译器、formatter、test、doc、package/CI 作为同一工具链；
- 对不同后端保持共享类型语义，并在后端边界明确报告限制；
- 快速反馈和高质量诊断优先于追求最大理论表达力。

不复制其他语言的具体语法或包格式。Calcit 的 Cirru/Snapshot 是自己的事实来源，演进必须保留
结构编辑和可迁移性。

## 设计不变量

未来类型功能必须满足：

1. 普通 typed core 不因 backend 不同获得不同的类型匹配结果；
2. FFI metadata、feature/effect 和 ABI transport 不污染普通 nominal type identity；
3. 无法证明时保留 Unknown evidence，不伪装为成功 specialization；
4. explicit Dynamic 永远可被 query/quality 解释其 intent；
5. 新规则先进入 preprocess/analyze，不向 interpreter 增加无关运行时特判；
6. 诊断包含 definition、path、expected、actual、evidence loss 和修复建议；
7. 新严格规则通过 baseline/entry policy 渐进启用；
8. native 与 JS 是公开语义后端，内部 WASM 验证后端不静默返回假结果。

## 路线 A：精度与 sound boundary

### A1. `Unknown/Unresolved` 内部状态

内部增加“不足以完成推断”的 evidence 状态，但不一定立即成为用户可写类型：

- 缺少 generic args；
- unbound TypeVar；
- unresolved type slot；
- 未知 imported schema；
- 分支无法合并。

它不能像 Dynamic 一样双向匹配所有类型。兼容模式可在最终边界降为 Dynamic，同时产生稳定诊断；
strict policy 下阻止公开 API、typed decoder 和 FFI 强类型 assertion 使用 unresolved value。

### A2. 显式 Dynamic intent

逐步让 intentional boundary 可由 schema/metadata 表达原因，例如 js-ffi、framework state、macro、
open data。quality 仍显示这些位置，但与 unresolved 使用不同策略。不要通过注释文本或文件路径
猜测 intent。

### A3. unsafe inventory

`unsafe-coerce`、raw host operation、native ABI assertion 进入统一 inventory。目标不是禁止 unsafe，
而是做到 Rust 式“边界很小、调用原因可查、测试责任清楚”。

## 路线 B：控制流与代数数据

### B1. 穷尽性与 Never

- named Enum match 检查所有 variants；
- duplicate/unreachable pattern 诊断；
- `Never` 表示必然终止、raise、todo trap 等无返回路径；
- 分支合并理解 Never，不要求用 Dynamic/nil 填补；
- Option/Result helper 与 match 保留 payload 精度。

### B2. Narrowing

只对可证明的语言结构做 flow-sensitive narrowing：

- Option/Result/Enum variant match；
- nil/nullish presence check；
- predicate 若有明确 compiler-known contract，可收窄对应分支；
- assertion 只在成功分支增加 evidence。

不引入任意 JavaScript property test 驱动的结构化 narrowing，也不从字符串比较推导复杂 union。

### B3. Data decoding

将严格 Cirru EDN decoder、Map-to-Struct 和未来 JS data decoder 统一建立在 closed data shape 上。
Dynamic、裸容器和 unresolved slot 不能生成一个看似安全的 decoder。

## 路线 C：局部推断与泛型体验

### C1. 双向检查

在已有 bottom-up inference 上增加 expected-type 向下传播，优先覆盖：

- lambda 作为已知 Fn 参数；
- Struct/Enum constructor payload；
- Option/Result method chain；
- collection literal 的元素/键值；
- callback return；
- match branch expected result。

目标是减少重复 annotation，而不是进行不可预测的全程序推断。

### C2. 泛型诊断

泛型失败需要解释：

- 哪个参数绑定了 TypeVar；
- 哪两个约束冲突；
- outer shape 是否仍被保留；
- 是否因 Dynamic/Unknown 丢失 specialization；
- 可选择补 annotation、泛型或 trait bound 的具体位置。

### C3. Trait coherence

先稳定 impl identity、conflict、selection 和 where-bound 诊断。generic trait、associated type 或
higher-kinded abstraction 只有同时满足以下条件才进入独立 RFC：

- 至少三个非 FFI 的真实生态用例；
- 不需要结构化自动满足；
- 能定义明确 coherence/overlap 规则；
- native/JS 语义和 query evidence 一致；
- 不显著恶化常见代码的推断耗时与错误可读性。

## 路线 D：框架与 effect

### D1. 框架类型表面

先在 Respo/Editor 试点：

- 标准化 Store、Reel、Dispatch、Component Props、Effect Callback；
- 用 generic Struct/Enum/Fn 保留 state/action 关系；
- framework intentional boundary 只留在入口和生命周期 adapter；
- 业务 component 内不依赖全局 Dynamic。

如果固定字段 props 的书写成本仍然过高，可以讨论 record/row ergonomics，但必须保留封闭字段、
名义导出和清晰错误；不直接采用 TypeScript 开放 object type。

### D2. Feature、effect 与 resource 分离

`:features` 表示实现体允许使用的 capability；未来 `:effects` 若存在，表示可传播的 effect；resource
若需要线性/生命周期约束，则是第三个独立概念。三者不能都塞进 Fn equality 或用一个 set 同时解释。

优先通过 effects graph、query 和 lint 积累用例，再决定是否进入函数类型。

## 兼容与采用

### Strict policy，而非全局硬切换

建议 entry/project policy 逐步提供：

- compatible：允许历史 Dynamic fallback，输出 evidence；
- ratchet：不得比提交的 quality baseline 增加债务；
- strict：公开 API、decoder、FFI typed boundary 不允许 unresolved；
- core：Calcit core/stdlib 使用的更强内部门禁。

名称可以在实现 RFC 中调整，但含义必须清楚。新模块从 strict/zero baseline 开始，历史应用使用
ratchet，避免每次类型增强引发全生态同步重写。

### 生态验证集

类型系统改动不能只通过 compiler fixtures。至少维护：

- 小型纯库：bisection-key、memof；
- 高覆盖 FFI：js-ffi、calcit-wss；
- native dylib：http/wss/fetch 类项目；
- 框架：recollect、Respo workflow；
- 大型应用：Editor、网站；
- native 与 JS 生成/运行；
- 内部 WASM subset regression。

每次 release 记录 quality baseline delta、诊断变化、后端矩阵和至少一个关键消费者结果。

## 明确暂缓的方向

- 通用 union/intersection/conditional types；
- TypeScript 式 structural object satisfaction；
- 任意 overload resolution；
- 由 JS FFI 单独驱动 generic trait/associated type；
- 没有资源用例支撑的 borrow checker；
- 全程序隐式 HM inference；
- 把 feature/effect/target 混入普通 Fn 匹配；
- 为每个后端维护不同的核心类型规则。

暂缓不等于永久拒绝；重新提出时必须带真实用例、迁移成本、诊断设计和多后端验证。

## 实施顺序

### Phase 1：保证边界诚实

- 区分内部 Unknown 与 intentional Dynamic；
- unresolved type slot/generic 不再静默 specialization；
- unsafe/FFI inventory；
- quality baseline 在参考模块落地。

### Phase 2：提高 typed core 表达力

- Enum exhaustiveness、Never 和 narrowing；
- lambda/constructor/callback 的双向检查；
- 泛型 binding evidence 与冲突诊断。

### Phase 3：框架试点

- Respo/Editor 的 typed state/props/dispatch；
- 观察是否需要 record ergonomics、generic trait 或 effect contract；
- 只有数据证明后才提交独立核心扩展 RFC。

### Phase 4：稳定性承诺

- strict policy 成为新模块默认；
- pure typed core 的 runtime type failure 建立 compiler bug 分类与 regression test；
- FFI/runtime/ABI 失败有独立 contract 错误和验证矩阵；
- 类型与诊断机器协议遵循兼容版本策略。

## 成功标准

1. 新模块不使用 Dynamic 也能自然表达常见业务和 callback 数据流。
2. intentional Dynamic 与 inference failure 在 query/quality 中完全可区分。
3. pure typed core 出现普通类型 runtime failure 时可以稳定归类为 compiler defect。
4. Editor/Respo 的 Dynamic 主要收缩到框架入口，而不是遍布业务 definition。
5. FFI 类型通过后仍由 target/contract tests 提供宿主证据，错误能够指回 Calcit binding。
6. 新类型特性不会让不使用它的程序改变 trait candidate 或 backend 行为。
7. 生态升级使用 baseline ratchet，不再依靠一次性大规模改写。

## 与既有 RFC 的关系

- `02-18-language-theory-evolution-plan.md` 继续描述 law、语义分层和类型驱动诊断愿景；本 RFC
  补充静态类型 sound boundary、推断、控制流和生态采用路线。
- `07-26-static-semantic-analysis-rfc.md` 是 evidence/diagnostic 基础。
- `08-05-systematic-nil-reduction-rfc.md` 是 Option/Result/Unit/Never 路线的具体迁移。
- `08-18-calcit-typed-js-ffi-boundary-rfc.md` 和
  `08-21-js-ffi-runtime-contract-validation-rfc.md` 负责宿主边界。
- `08-21-type-quality-ci-adoption-rfc.md` 负责把类型演进安全地推入生态。

