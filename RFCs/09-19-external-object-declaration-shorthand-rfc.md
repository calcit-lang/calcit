# RFC：external-object trait 的简写声明形式

状态：Partial

日期：2026-09-19

关联：

- `RFCs/08-18-calcit-typed-js-ffi-boundary-rfc.md`（JS FFI 边界主体模型）
- `docs/features/js-interop.md`（当前用法与诊断）
- [#1223](https://github.com/calcit-lang/calcit/issues/1223)（本 RFC 的实现跟踪）

> P1 已落地：`defexternal` 在 Snapshot 载入期规范化为 `deftrait` 代码 + `:ffi` 元数据
> （`src/snapshot.rs` 的 `normalize_defexternal_code` / `normalize_entry_external`），并带有稳定错误。
> P2 已落地：`calcit edit def` 可直接写入 `defexternal` 简写，写盘 schema 归一化为 `:: 'Trait`
> （`normalize_schema_for_code` 识别 `defexternal`），且写盘前用 `validate_defexternal_shorthand`
> 快速失败。P3（可选保留源码头）仍待评估。

## 摘要

当前声明一个 JS external-object trait 需要两步：写 `deftrait` 代码，再在同一 `CodeEntry` 上补
`:ffi` 元数据（`:backend`/`:kind`/`:names`/`:writable`/`:target`）。字段/方法契约与宿主名映射、
可写集合被拆到两处，样板较重，不利于在适配器里快速建立最小契约，也不利于自动转写。

本 RFC 提案一个等价的简写声明形式 `defexternal`：在一处同时表达 trait 成员与 FFI 元数据，
lowering 为与今天完全相同的 `deftrait` 代码 + `:ffi` 元数据。**不引入新的类型变体，不改变普通 trait
的 method candidate、`impl-traits`、泛型统一与 `:where` 语义**，也不改变 `calcit query` 的归一化输出。

## 背景与现状

已落地能力（见 RFC 08-18 与 `docs/features/js-interop.md`）：

- `deftrait` 用 tag member 描述宿主字段、method member 描述宿主方法；
- `CodeEntry :ffi` 保存 `:backend :js`、`:kind :external-object`、`:names` 宿主名映射、
  `:writable` 可写字段与 `:target` 宿主目标；
- 静态派发：`(receiver .method args...)` 映射到宿主 property/method；
- `js-get`/`js-set` 在 receiver 与 key 静态时使用 trait 字段契约；
- 严格诊断：`E_UNTYPED_JS_OBJECT_ACCESS`、`E_JS_FFI_NULLABLE_DEREF` 等；
- 审计：`--warn-dyn-method`、`calcit analyze weak-types --ffi-evidence`。

一个最小 external-object trait 目前写作（示意，`:ffi` 是 `CodeEntry` 元数据，不是代码）：

```cirru.no-check
deftrait QueryHost
  :length 'Number
  .query $ :: 'Fn
    {}
      :args $ [] 'QueryHost 'String
      :return 'QueryHost
  .text $ :: 'Fn
    {}
      :args $ [] 'QueryHost
      :return 'String

:ffi $ {}
  :backend :js
  :kind :external-object
  :target :browser
  :names $ {} (:query |querySelector)
  :writable $ #{} :text-content
```

可用工具：

- 代码侧：`calcit edit def` / `calcit tree` 编辑 trait 成员；
- 元数据侧：`calcit edit ffi <target> --code '...'`、`--clear`；
- 查询侧：`calcit query def <target> --json` 返回归一化后的 `ffi` 元数据。

## 问题

1. **声明分散**：字段/方法契约在代码里，`:ffi` 在元数据里，建立最小契约需要在两个入口间切换。
2. **模板重复**：常见形态（`?`/`!` 后缀、kebab-case 到 camelCase 的宿主名映射）每次都手写 `:names`。
3. **审计与转写成本**：Agent 在建立或补全 external trait 时，容易漏 `:kind`/`:target`/`:writable`，
   或生成与既有 `deftrait` 不一致的结构。
4. **迁移效率**：从裸 `JsObject` 收敛（#1224）时，若有一处可表达完整契约，自动建议更容易落地。

## 目标

- 提供一个 `defexternal`（或等价）的**声明形式**，在同一处表达成员与 FFI 元数据。
- lowering 产物与手写 `deftrait` + `:ffi` **逐字段等价**，下游类型系统、codegen、query 无需新增分支。
- `calcit query def --json` 仍返回与今天一致的归一化 `ffi` 元数据。
- 提供稳定错误与诊断，覆盖字段/方法/schema 与元数据组合的非法情况。
- 允许后续 `calcit fix`/`edit` 在 `deftrait`+`:ffi` 与 `defexternal` 之间做可证明的转换。

## 非目标

- 不引入泛型宿主 trait（`Promise<T>`/`Iterator<T>`/`Event<Target>`）；按 RFC 08-18，适配器在边界消费。
- 不引入结构类型、联合类型、重载或 DOM 继承树。
- 不改变普通 `deftrait` 的语义、字段可写性模型或 `:where`/泛型求解。
- 不改变外部值模型：宿主值仍是不透明对象，字段默认只读，`unsafe-coerce` 仍只在适配器边界。
- 不新增顶层 CLI 命令。

## 设计原则

- **语法糖，不是新类型**：`defexternal` 只是 authoring 便捷入口，产物仍是 `deftrait` + `:ffi`。
- **一处表达，可审计**：成员、宿主名映射、可写集合与 target 必须能从 Snapshot 与 `calcit query` 还原。
- **默认最小**：默认字段只读；默认方法名映射沿用现有规则；只有显式声明才扩展。
- **可往返**：简写与展开形式之间应能结构化互转，不丢信息（不靠文本 patch）。

## 语法提案

`defexternal` 接受：名称、保留的 external 选项、以及若干 trait 成员。选项先于成员：

```cirru.no-check
defexternal QueryHost
  :target :browser
  :names $ {} (:query |querySelector)
  :writable $ #{} :text-content
  :length 'Number
  .query $ :: 'Fn
    {}
      :args $ [] 'QueryHost 'String
      :return 'QueryHost
  .text $ :: 'Fn
    {}
      :args $ [] 'QueryHost
      :return 'String
```

规则：

- 保留选项（大小写敏感）：
  - `:target`：`:browser` | `:node` | `:native` | `:wasm`（与 entry/`:ffi :target` 一致）；
  - `:names`：`{calcit-name host-name}`，键为字段 tag 或方法符号，覆盖默认映射；
  - `:writable`：字段 tag 集合，默认空（只读）；
  - `:backend`：可省略，external-object 固定为 `:js`；显式传入其他值报错。
- 成员语法与 `deftrait` 一致：`:field Type` 声明字段，`.method Schema` 声明方法；方法 schema 的
  `:args` 首个位置必须是本 trait（self），与今天相同。
- `:kind` 不在简写里出现：`defexternal` 固定产出 `:kind :external-object`。
- 成员的默认宿主名映射沿用现有规则（例如 `text-content → textContent`、`matches? → matches`、
  `set-item! → setItem`）；`:names` 只覆盖例外。

## Lowering 与 Snapshot 表示

**MVP 采用规范化 lowering**：`defexternal` 是 authoring 简写，读取后规范化为
`deftrait` 代码 + `:ffi` 元数据；`calcit edit` 与 `calcit edit format` 写回规范形式。

- 代码：生成与等价 `deftrait` 相同的 `:code`（成员顺序保持声明顺序）；
- 元数据：生成

  ```cirru.no-check
  :ffi $ {}
    :backend :js
    :kind :external-object
    :target :browser
    :names $ {} (:query |querySelector)
    :writable $ #{} :text-content
  ```

- 载入路径：当 Snapshot 的某个 definition `:code` 以 `defexternal` 开头时，载入阶段（surface
  normalization）就地展开为 `deftrait` + `:ffi`，再进入既有 preprocess。若同时已存在 `:ffi`
  元数据，以显式元数据为准并给出确定性冲突错误或合并规则（见“错误行为”）。

备选方案（见“备选方案”）：保留 `defexternal` 源码头，在下游分别处理；或只提供 `calcit edit`
入口而不允许手写源码。MVP 选择规范化 lowering，理由是下游零改动、与现有 `calcit edit ffi` 一致、
且符合“表层语言拥有用户语义、typed core 只做保持语义的解析与 lowering”的分层约定。

## 类型系统与 query 行为

- `defexternal` 产出的 definition 必须是 `Trait` schema，与 `deftrait` 相同；类型标注仍直接引用该名字。
- 外部字段读取、`js-get`/`js-set`、`(receiver .method ...)` 的类型行为与今天完全一致。
- `calcit query def <target> --json` 的 `ffi` 字段保持今天的归一化形状；
  `calcit query def` 人类输出仍打印 `FFI:` 行。
- `E_UNSCOPED_UNSAFE_COERCE`、`E_JS_FFI_TARGET_MISMATCH`、`W_JS_FFI_FIELD_READONLY` 等诊断沿用。

## 兼容性

- 既有 `deftrait` + `:ffi` 声明完全不受影响；`defexternal` 是增量入口。
- 已发布模块若使用手写元数据，载入后行为不变。
- `calcit edit format` 不重写用户手写的 `deftrait` + `:ffi`；只有含 `defexternal` 的 definition
  会被规范化。

## 错误行为

稳定错误（给出 definition 与字段/方法定位）：

- 保留选项重复或未知（例如同时写两处 `:target`，或未知 `:writable-x`）；
- `:backend` 非 `:js`（或与 external-object 冲突）；
- 成员既不是 `:field` 也不是 `.method`；
- 方法 schema 缺少 self 参数或 self 类型不是本 trait；
- 字段/方法重名，或 `:writable` 引用了不存在的字段；
- 简写与既有 `:ffi` 元数据不一致（显式元数据存在时按“显式优先 + 冲突报错”处理，不做静默合并）。

## 备选方案

1. **给 `deftrait` 增加 `:external` 元数据子句**：能保持单一入口，但会改变 `deftrait` 解析与既有
   成员语法，且把宿主专用概念塞进普通 trait 表面，违反 RFC 08-18 的“不把 FFI 特例注入核心 trait”。
2. **core macro 展开**：macro 无法写 `CodeEntry :ffi` 元数据，且会把宿主概念带进普通 macro 展开链，
   不满足“成员与元数据一处表达”。
3. **只提供 `calcit edit add-external` CLI**：可立即减少样板，但不是可在 Snapshot 源码中阅读与 review 的
   *声明形式*；本 RFC 要求可手写、可版本化的声明。
4. **保留 `defexternal` 源码头，下游分别处理**：authoring 体验最好，但 preprocess/type/codegen/query
   都要新增分支，MVP 成本高；可作为后续演进（见开放问题）。

## 开放问题

1. 是否长期保留 `defexternal` 源码头（而非规范化展开），以获得最佳 authoring 与 diff 体验？
2. `:names`/`:writable` 是否需要在 `query` 人类输出中直接展示默认映射后的宿主名？
3. 是否允许在 `defexternal` 中声明字段默认值或 optional 字段（会引入新的语义）？
4. `calcit fix` 双向转换规则的适用范围与 review-only 边界（与 #1224 协同）。

## 验收与测试

- 解析/规范化：
  - `defexternal` 展开后与等价手写 `deftrait` + `:ffi` 的 Snapshot 结构逐字段一致（Rust round-trip 测试）；
  - 成员顺序、`:names` 覆盖、`:writable` 集合在展开中保持。
- 类型系统：
  - 展开后 `assert-type`/方法派发/`js-get`/`js-set` 行为与手写形式一致（Calcit definition `:tests`）；
  - 普通 `deftrait`（无 external）行为不变。
- query：`calcit query def --json` 返回归一化 `ffi`，`calcit query` 人类输出含 `FFI:` 行。
- 错误：上述非法组合各返回稳定错误与定位。
- 文档：`docs/features/js-interop.md` 与 `docs/run/fix.md` 更新示例，`calcit docs check-md` 通过。
- 边界：不新增顶层命令；不自动插入 `unsafe-coerce`、不扩大 `Dynamic`、不猜测字段类型或宿主名。

## 实施阶段

1. **P1（本 RFC 落地）**：Snapshot 载入期 `defexternal` → `deftrait` + `:ffi` 规范化；解析错误；
   round-trip 与类型一致性测试；文档。
2. **P2**：`calcit edit` 增加结构化入口（创建/更新 external trait），复用 P1 的 lowering，
   保证 `calcit query` 输出不变。
3. **P3（可选，取决于开放问题 1）**：在确认 authoring/diff 价值后，评估保留源码头并让下游识别；
   以及 `calcit fix` 在两种形式间的可证明转换（与 #1224 协同）。
