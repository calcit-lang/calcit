# RFC：JS FFI 静态声明与运行时契约验证

状态：Draft

日期：2026-08-21

## 摘要

Calcit 的 JS FFI 已能用 `JsObject`、`JsNullish<T>`、完整 Fn schema、external-object trait
和 `:features #{:js-ffi}` 描述边界，但“类型检查通过”目前主要证明 Calcit 侧的声明自洽，不能
证明 JavaScript 宿主真的满足声明。

本 RFC 补充静态类型与实际宿主之间的验证闭环：

1. 将 host import、property/method、callback、exception、Promise 和 `unsafe-coerce` 视为需要
   单独证据的 contract boundary；
2. 在 `js-ffi` 库提供可复用的 guard/decoder/normalizer，不让每个应用自行断言；
3. 为 Node 与 browser 建立正向、负向 runtime contract test；
4. 将 unchecked host assertion 纳入原生质量报告，但不把 runtime test 冒充静态分析；
5. 保持现有名义类型与 FFI metadata 设计，不引入 TypeScript 式结构类型系统。

本 RFC 是 `08-18-calcit-typed-js-ffi-boundary-rfc.md` 的后续。前者解决“如何声明和授权”，
本 RFC 解决“声明如何被宿主运行证据支持”。

## 问题定义

下面的代码可以在静态层拥有精确返回类型：

```cirru.no-check
defn viewport-width () $ unsafe-coerce js/window.innerWidth Number
```

但 schema 本身不能证明：

- 当前 target 一定有 `window`；
- `innerWidth` 存在且是 number；
- npm 包仍导出声明中的 symbol；
- 取出的函数是否需要 JavaScript `this`；
- callback 会收到声明的参数；
- Promise 不会 reject；
- `null`/`undefined` 没有被误断言为普通值；
- 已生成代码与当前 `@calcit/procs` runtime 相容。

因此应明确：

> FFI schema 是 Calcit 对宿主的契约声明，不是宿主事实的自动证明。

在纯 Calcit typed code 中，“类型通过但发生普通类型错误”应被视为编译器问题；在 FFI 边界，
同类失败首先说明 contract 没有被验证、第三方 API 漂移，或使用了显式 unsafe escape。

## 失败分类

| 类别 | 例子 | 首选发现阶段 |
| --- | --- | --- |
| target mismatch | browser binding 用于 Node | codegen 前静态检查 |
| missing binding | npm named export 被移除 | module load/runtime contract test |
| wrong primitive | 声明 Number，宿主返回 String | boundary guard |
| nullish mismatch | `undefined` 被断言成 String | boundary guard/decoder |
| object capability | method 不存在或不是 function | external-object assertion/test |
| receiver loss | 取出 method 后无 `this` 调用 | lowering test |
| callback mismatch | 宿主参数或 callback 返回值不符 | callback adapter contract test |
| exception/rejection | API throw 或 Promise reject | adapter 转 Result + negative test |
| runtime identity | 生成代码引用旧/缺失 proc | JS runtime identity test |

静态分析应尽早拒绝可证明的问题；需要观察宿主值的问题由 boundary validation 和 runtime test
承担。不能因为后者无法在编译期证明，就整体退回 `Dynamic`。

## 设计原则

### unsafe 必须可见

`unsafe-coerce` 继续表示“维护者提供可信证据”，不偷偷增加运行时开销，也不伪装成 decoder。
但 compiler/query/quality 必须能够报告：

- source type；
- target type；
- definition 与 Snapshot path；
- 所在函数是否具有 `:js-ffi`；
- 是否位于被声明为 raw adapter 的 namespace/definition；
- 是否紧邻一个已识别 guard/decoder。

从宿主值直接 coercion 到 Struct/Enum、primitive 或精确 Fn schema 是高风险项。它可以存在，但
必须被审计，且不能在普通业务 namespace 扩散。

### guard、decoder、normalizer 分层

`js-ffi` 提供三类可复用能力：

1. **guard**：验证 primitive、nullish、array、function、必要字段/方法等浅层宿主事实；
2. **decoder**：递归验证外部数据并返回 `Result<T, JsError>`；适合 JSON/object 数据；
3. **normalizer**：捕获 exception/rejection，把宿主对象转换为 Struct/Enum/List/Map/Option/
   Result/Unit。

具体 API 名称可以在实现时确定，但语义必须区分：guard 证明一个有限事实，decoder 产生
Calcit-owned data，`unsafe-coerce` 只接受维护者断言。

`decode-map-as` / 严格 Cirru EDN decoder 已证明“按目标类型派生验证器”可行；JS object decoder
应尽量复用同一 data-shape 规则和错误路径格式，而不是在 js-ffi 内维护另一套类型 AST。

### external trait 是能力，不是完整对象验证

external-object trait 声明调用者会使用的稳定字段和方法。对一个 `JsObject` 建立该 trait
evidence 时，测试/调试模式至少可以验证：

- 声明为 method 的成员存在且为 function；
- 必需字段存在；
- 可廉价验证的 primitive 字段类型正确；
- writable metadata 不包含未知字段。

这不是 TypeScript structural satisfaction，也不生成普通 runtime impl。宿主对象后续仍可能
变化；真正需要稳定数据不变量时应复制并 decode 成 Struct/Enum。

### exception 与 async 是契约的一部分

可能 throw/reject 的 API 不得只声明成功返回类型。公共 adapter 返回
`Result<T, JsError>`；不存在与失败分别用 `Option` 和 `Result` 表达。未 await 的 Promise 不以
裸 `JsObject` 泄漏到业务层。

## 编译器与分析改进

### Host contract evidence

在现有 `HostOperation`/FFI metadata 基础上，为边界产生统一 evidence：

- binding target 与 host name；
- operation kind：import/value/property/method/constructor/callback/await；
- logical schema；
- target：browser/node/neutral；
- assertion kind：checked/decoded/unsafe；
- definition/path。

它是类型检查后的 contract analysis，不参与普通 `matches_with_bindings`、泛型统一或 trait
candidate selection。

### 诊断

建议新增或稳定以下诊断：

- `E_JS_FFI_TARGET_MISMATCH`：selected entry 与 binding target 冲突；
- `E_JS_FFI_FEATURE_REQUIRED`：实现体没有授权 host operation；
- `W_JS_FFI_UNCHECKED_COERCE`：宿主值直接转换到强类型且没有可见验证；
- `W_JS_FFI_UNTESTED_BINDING`：公共 binding 没有 contract example/test evidence；
- `E_JS_FFI_RUNTIME_CONTRACT`：调试/测试 guard 发现实际值与声明不一致。

runtime error 至少携带 Calcit definition、host binding/member、expected、actual kind 和 entry
target。不要只暴露 JavaScript `Cannot read properties of undefined`。

### Quality 集成

`analyze quality` 后续增加独立维度，例如 unchecked host assertions 与 untested public
bindings。它们进入同一版本化 JSON/baseline 协议，不由项目自写 JS 扫描 generated code。

quality 输出只声明“发现/未发现静态契约风险”，不能声称 Node/browser runtime tests 已执行。
后端 test 仍是单独 CI step。

## `js-ffi` 库改进

### 模块组织

建议稳定三层目录/namespace：

- `js-ffi.raw.*`：唯一允许 raw `js/*` 和必要 `unsafe-coerce` 的实现层；
- `js-ffi.host.*`：小型 external traits 与 checked host wrappers；
- `js-ffi.*` / `js-ffi.types`：normalized public API、`JsError` 和业务可用数据。

已有公开路径可以通过 re-export 保持兼容，不要求一次性改名。

### 优先补强的应用场景

1. `globalThis`、Node `process`、browser `window/document` 的 target detection；
2. npm/ES module function import，特别是 default/named export 与 receiver method；
3. DOM query、event target 和 listener callback；
4. timer、storage、clipboard 等 effect API 的 Unit/Result 契约；
5. Promise resolve/reject 与 async callback；
6. host Array/object 到 `List<T>`、Struct、Enum 的 decoder；
7. `@calcit/procs` generated-code runtime identity。

每个场景先选真实应用调用路径，不为未使用的完整 DOM/npm surface 建模。

## Runtime contract test 矩阵

### Node

- 生成实际 JS；
- 使用项目声明的 `@calcit/procs` 版本执行；
- 调用每个公共 Node binding 的最小成功用例；
- 对 missing export、错误 primitive、throw、rejection 和 nullish 运行负向 fixture；
- 验证错误包含 Calcit binding identity，而不只是原生 JS stack。

### Browser

- 在 headless browser 装载真实生成物；
- 对 DOM property/method、receiver binding、event callback 和 writable field 做 smoke test；
- 在没有目标 API或返回 nullish 时验证 Option/Result；
- Node-only binding 在 browser target 于 codegen 前失败，反向亦然。

### Generated runtime identity

- codegen 使用的 proc export 在当前 `@calcit/procs` 中全部存在；
- compile-time-only form 不泄漏为 runtime proc；
- Calcit CLI 与 npm runtime 版本不同时给出明确兼容错误或由项目版本策略阻止；
- contract test 执行 generated JS，而不是只检查文本中是否出现某个 symbol。

### Callback

至少覆盖：

- primitive/Struct callback 参数正常；
- nullish 参数按声明转换；
- callback 返回 Unit；
- callback throw 被保留为明确错误或按 adapter 契约转 Result；
- async callback rejection；
- escaping callback 的 `:js-ffi` lexical capability 没有丢失。

## 运行时检查策略

不是所有 external field access 都需要永久重复 `typeof`。建议分三种模式：

| 模式 | 行为 | 使用场景 |
| --- | --- | --- |
| boundary | 只为一次 decoder 操作检查并立即复制为 Calcit-owned data | 默认生产路径 |
| debug | boundary 外加关键 callback/返回值断言 | 测试、开发、生态升级 |
| unsafe | 只保留明确 `unsafe-coerce`，由审计和 contract test 承担 | 性能敏感且已证明的 adapter |

模式属于 entry/build policy，不进入普通类型身份。即使选择 unsafe，target 和 capability gate 仍需
静态检查；关闭 runtime guard 不等于允许未授权 FFI。

### evidence 的有效范围

host object 默认是可变的，不能把一次检查当成可无限期复用的“已验证外部对象”能力。boundary
模式的 evidence 仅覆盖当前 decoder 调用：decoder 必须立即读取所需字段并返回 Struct、Enum、
Option、Result 或其他 Calcit-owned data；调用方不能凭该 evidence 在稍后的代码中直接访问原
对象。若 adapter 必须保留外部对象或在稍后调用其方法，每一次 field/method access 都要重新
检查存在性和 primitive/function shape，或改为 debug 模式提供这类断言。external-object trait
仍只提供静态成员声明与代码生成映射，不证明运行时对象不可变或始终符合该 shape。

unsafe 模式可以有意跳过这些检查，但必须把 `unsafe-coerce` 保留在最小 adapter 中，并由针对
该 binding 的正向和负向 runtime contract test 提供可追溯证据。

## 实施阶段

### Phase 0：真实失败目录

- 从 js-ffi、Editor、Respo workflow 和网站项目收集“静态通过但 runtime 失败”的最小 fixture；
- 按 target/binding/value/callback/exception/runtime identity 分类；
- 为已有 public wrapper 标记 raw/host/normalized 层。

### Phase 1：库级 guard 与 contract tests

- 统一 `JsError`、primitive/nullish/function guards 和 object decoder；
- Node contract test 覆盖 module import、process、exception、Promise；
- browser contract test 覆盖 DOM、event、receiver 和 storage；
- effect wrapper 收敛为 Unit 或 Result。

### Phase 2：编译器 evidence 与诊断

- query/analyze 暴露 host contract evidence；
- unchecked coercion 进入 weak/quality report；
- target mismatch 在 codegen 前失败；
- runtime contract error 附带 Calcit definition/path。

### Phase 3：生态采用

- js-ffi 自身达到 Q3；
- Editor、Respo workflow 和网站各选择至少一个真实 browser consumer 回归；
- 新 public FFI binding 必须同时提交 schema、contract test 和 normalized API 决策。

## 验收标准

1. 每类已知“类型通过但 runtime 失败”都有最小正向和负向 fixture。
2. public js-ffi wrapper 不以无理由 Dynamic 隐藏 primitive/nullish/callback 关系。
3. 从 host object 到 Struct/Enum 的路径经过 decoder 或被明确报告为 unsafe。
4. Node/browser target mismatch 在运行前失败。
5. module method 的 receiver lowering 有真实执行测试。
6. throw/rejection/nullish 分别映射为 Result/Option，而不是偶然的 undefined/Dynamic。
7. quality 能报告 unchecked host assertion，但不伪造 runtime-test 状态。
8. 不使用 JS FFI 的程序在类型推断、trait matching 和 codegen 上不受影响。

## 非目标

- 完整导入 TypeScript `.d.ts`；
- 通用 structural object/union/overload 类型；
- 运行时深度验证每一次宿主属性读取；
- 用 FFI 推动 generic trait、associated type 或 effect row；
- 保证任意第三方 JavaScript 包升级都不会破坏契约。

## 相关资料

- `RFCs/07-08-ffi-features-and-js-object-type-rfc.md`
- `RFCs/07-31-unsafe-coerce-driven-static-type-boundary-plan.md`
- `RFCs/08-08-cross-backend-host-ffi-contracts-rfc.md`
- `RFCs/08-18-calcit-typed-js-ffi-boundary-rfc.md`
- `RFCs/08-21-type-quality-ci-adoption-rfc.md`
- `docs/features/js-interop.md`
- `docs/data/edn.md`
