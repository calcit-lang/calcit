# RFC: Cirru EDN 严格类型化反序列化

状态：Implemented / Phase 1 + DataShape 内核（非递归类型）
日期：2026-08-04
关联：`07-31-unsafe-coerce-driven-static-type-boundary-plan.md`、`06-01-generic-binding-unification-rfc.md`、`05-31-generic-where-bounds-mfs.md`、Issue #295

## 1. 摘要

新增语言级类型边界：

```cirru
parse-cirru-edn-as text TypeExpr
```

它把 Cirru EDN 文本直接解码为满足 `TypeExpr` 的 Calcit 值，并保证：只要调用成功，返回值的完整递归结构、容器元素、struct 字段、enum variant/payload、泛型实参以及名义类型身份都符合目标类型。

该操作不等价于：

```cirru
unsafe-coerce (parse-cirru-edn text) TypeExpr
```

后者只建立静态声明，不提供运行时证明；新操作则必须完成实际解析、验证与名义值构造，建立 `validated` 类型证据。

现有 `parse-cirru-edn` 保持兼容，继续作为动态数据接口。严格 API 不允许以 `Dynamic` 作为逃生口；确实需要任意 EDN 时，应显式使用旧动态 API。

## 2. 问题

当前 `parse-cirru-edn` 的返回类型是 `Dynamic`。它的第二个 options map 只根据名称把 record/enum 重新连接到声明对象：

- 不验证 List/Map/Set 内部元素；
- 不验证 record 字段值的声明类型；
- 不验证 enum variant 和 payload 类型；
- 不验证泛型实参与 `:where` 约束；
- record 字段集合不一致时存在不可恢复的 `unreachable!` 路径；
- 调用方即使紧接 `assert-type`，当前浅层匹配也不能形成深度证明。

因此 options map 解决的是“名义身份恢复”，不是“类型化反序列化”。把返回值交给 `unsafe-coerce` 只会隐藏边界风险。

## 3. 设计原则

1. **动态与已验证边界分离**：动态解析和严格解码使用不同名字、不同静态语义。
2. **禁止隐式退化**：严格解码图中不存在 `Dynamic`、未绑定类型变量或未知自定义节点。
3. **直接构造目标值**：严格解码从 `cirru_edn::Edn` 直接生成目标 Calcit 值，不先生成动态值再做浅检查。
4. **名义身份精确**：struct/enum 使用编译器解析到的实际声明对象，不能仅凭同名结构冒充。
5. **Native/JS 同一规则**：两个后端使用同构的解码图和错误路径语义。
6. **失败可定位**：错误包含目标类型与数据路径，例如 `$.friends[2].age`。
7. **旧 API 兼容**：本 RFC 不改变 `parse-cirru-edn` 的现有成功结果；迁移由调用方逐处完成。

## 4. 表面语法与静态语义

### 4.1 基本形式

```cirru
parse-cirru-edn-as raw Person

parse-cirru-edn-as raw $ :: 'List 'Number

parse-cirru-edn-as raw $ :: Box 'String
```

`TypeExpr` 使用现有类型表达式规则。命名 struct/enum 推荐直接使用声明值（如 `Person`、`Box`）；内建类型与泛型应用继续使用现有 `::` 表达方式。

该形式是 syntax，而不是普通 proc，原因有三点：

- 编译器必须在运行前解析并验证 `TypeExpr`；
- 返回类型就是 `TypeExpr`，不能退化为 proc 的固定 `Dynamic`；
- JS codegen 需要把类型解析结果编译为后端无关的解码图。

### 4.2 允许的目标类型

Phase 1 支持：

- `Unit`、`Bool`、`Number`、`String`、`Symbol`、`Tag`、`Buffer`、`CirruQuote`；
- `Optional<T>`；
- `List<T>`、`Set<T>`、`Map<K,V>`、`Ref<T>`；
- 具有完整字段类型的 `defstruct`；
- 具有完整 variant/payload 类型的 `defenum`；
- 上述类型的有限组合；
- 所有泛型实参均已给出的 struct/enum 应用。

Phase 1 明确拒绝：

- `Dynamic`，以及裸 `List`/`Map`/`Set`/`Ref` 所隐含的 Dynamic 参数；
- 未绑定 `TypeVar`、未解析或未绑定的 `TypeSlot`；
- 泛型参数缺失、过多或不满足 `:where`；
- `Fn`、proc、trait、impl、`JsObject`、任意宿主引用；
- 未知 `Custom` 类型；
- 没有 enum 声明身份的普通 tuple；
- `Variadic<T>`（它是函数参数约束，不是数据类型）。

拒绝发生在预处理/编译阶段。动态输入不能迫使编译器生成“遇到不懂的节点就接受”的解码器。

### 4.3 返回类型

静态分析把整个表达式推断为已经解析、完成泛型替换后的 `TypeExpr`，其证据等级为 `validated`。Phase 1 先把类型本身接入现有推断；证据等级的结构化查询等后续静态分析基础设施具备后再暴露。

### 4.4 错误模型

为保持与 `parse-cirru-edn` 一致，Phase 1 在失败时抛出可被 `try` 捕获的错误，而不是额外引入标准库 `Result` 依赖。

错误至少包含：

- 错误类别：文本解析失败、kind 不匹配、名义名称不匹配、字段集合不匹配、variant 不存在、payload arity 不匹配、值类型不匹配；
- 目标类型；
- 结构路径；
- 实际 EDN kind 或名称。

示例：

```text
parse-cirru-edn-as failed at $.friends[2].age: expected number, got string
```

## 5. 严格构造规则

### 5.1 标量和容器

- 标量必须匹配对应 EDN variant，不做字符串到数字等隐式转换；
- `Optional<T>` 只把 EDN `nil` 解释为缺失，否则按 `T` 解码；
- List/Set/Map 对每个元素或键值递归解码；
- `Ref<T>` 只接受 EDN `atom`，并对 atom 内部值递归解码；
- Map key 与 value 都必须满足各自目标类型。

### 5.2 Struct

- 输入必须是 `%{}` record，不能把普通 map 自动提升为 struct；
- EDN record 名必须和声明名一致；
- 字段集合必须精确一致：未知字段和缺失字段都是错误；
- 不以 `nil` 代替缺失字段，不读取默认值；
- 字段值按泛型替换后的字段类型递归解码；
- 成功值的 `struct_ref` 必须是目标声明对象，保留其 trait/impl。

### 5.3 Enum

- 输入必须是 `%::` enum tuple，普通 `::` tuple 不自动升级；
- EDN enum 名必须和声明名一致；
- variant 必须存在；
- payload arity 必须精确一致；
- payload 按泛型替换后的类型逐项递归解码；
- 成功值的 `sum_type` 必须是目标 enum 声明对象。

### 5.4 泛型和 `:where`

对 `Box<String>`：

1. 检查 `Box` 的泛型参数数量；
2. 建立 `{T -> String}` 绑定；
3. 检查 `T` 对应的 `:where` trait 约束；
4. 把字段/variant 中的 `T` 替换为 `String`；
5. 对替换后的图递归执行同样的可解码性检查。

泛型函数内部不能依赖运行时反射出 `T`。后续公开 `EdnDecoder<T>` 后，泛型函数必须显式接收 decoder dictionary。

## 6. 共享的闭合数据形状

严格 EDN 解码已经提炼为编译器内部的 `DataShapeGraph`。EDN 执行器只负责“怎样从 EDN 读取节点”；名称解析、泛型替换、`:where` 检查和名义类型绑定都只在 shape 构建阶段执行一次。后续 JSON、FFI、持久化和 diff/patch 必须复用该 shape，不能各自实现一套不完整的类型反射。

### 6.1 当前 ABI

```text
DataShapeGraph {
  version: 1
  root: NodeId
  fingerprint: MD5(ABI version + complete normalized graph)
  nodes: [DataShapeNode]
}

DataShapeNode =
  Unit | Bool | Number | String | Symbol | Tag | Buffer | CirruQuote
  Optional(NodeId)
  List(NodeId)
  Set(NodeId)
  Map { key: NodeId, value: NodeId }
  Ref(NodeId)
  Struct {
    nominal_path: Namespace/Definition
    type_args: [ClosedType]
    fields: [(FieldTag, NodeId)]
  }
  Enum {
    nominal_path: Namespace/Definition
    type_args: [ClosedType]
    variants: [(VariantTag, [NodeId])]
  }
```

`fingerprint` 覆盖 ABI 版本、root、所有节点、名义声明路径、完整泛型实参、字段顺序和 variant payload。它不是安全散列或输入签名，而是防止把一个类型的编译产物误用于另一个 schema 的兼容性标识。

构建阶段拒绝 Dynamic、未绑定变量、不完整泛型、失败的 where-bound、未知 custom 类型和递归 slot。每个 child node id 在 graph 完成时校验。Phase 1 暂不接受递归类型；node id 表示使未来增加 cycle-safe 占位节点时不必改写消费者模型。

关键不变量是：

> `DataShape<T>` 构建成功，表示 `T` 是一个可由编译器完整遍历并精确重建名义身份的闭合数据类型。

### 6.2 数据边界如何复用 shape

每一种边界由两部分组成：共享 shape + 格式策略。

| 能力 | 共享部分 | 格式专有部分 |
| --- | --- | --- |
| Cirru EDN decode | `DataShape<T>` | EDN kind、record/enum 编码与错误路径 |
| JSON decode | `DataShape<T>` | tag、map key、enum 的 JSON 表示策略 |
| FFI/message decode | `DataShape<T>` | 宿主值读取与资源限制 |
| typed patch | `DataShape<T>` | patch 操作、冲突和集合 diff 策略 |

新边界要么返回经过完整 shape 验证的 `T`，要么明确返回 Dynamic。禁止以浅层 `assert-type` 或只恢复 record 名称的方式把动态值升级成 `T`。

当前不公开可伪造的普通 record 形式 `DataShape<T>`。一等 `EdnDecoder<T>` / `DataShape<T>` 需要先有编译器派生、不可伪造的泛型 dictionary ABI；在此之前 `parse-cirru-edn-as` 继续是公开入口。

### 6.3 typed struct 更新是 patch 的前置能力

typed patch 不能建立在返回宽泛 `Record` 的 `assoc` 上。对于已知 `Struct<A...>`，核心语言必须保证：

1. 字段存在；
2. 写入值满足泛型替换后的字段类型；
3. `assoc` / `with` 返回原接收者的精确名义类型和泛型实参；
4. 编译器可把字段 tag 改写为位置索引，运行时仍保留 tag 作为一致性检查。

当前实现已覆盖以上四点：预处理按替换后的字段类型检查并生成 index/tag，Native indexed runtime 也会重新核对非负整数 index 与 field tag，防止旧 patch 在 schema 漂移后静默写入相邻字段。这一能力应成为所有 typed patch apply 的唯一 struct 写入路径。

## 7. `Patch<T>` 的具体协议

Recollect 当前的 `diff-twig` / `patch-twig` 继续处理开放数据树，其 Dynamic 签名是合理的兼容 API。静态业务状态另行使用不可伪造的 `Patch<T>`：

```text
Patch<T> {
  shape_version: 1
  shape_fingerprint: String
  root: PatchNode
}

PatchNode =
  Keep { node: NodeId }
  Replace { node: NodeId, value: TypedValue(node) }
  StructFields { node: NodeId, fields: [(FieldIndex, PatchNode)] }
  EnumPayload { node: NodeId, variant: VariantIndex, payloads: [(PayloadIndex, PatchNode)] }
  ListOps { node: NodeId, ops: [TypedListOp] }
  MapOps { node: NodeId, ops: [TypedMapOp] }
  SetOps { node: NodeId, ops: [TypedSetOp] }
  RefValue { node: NodeId, patch: PatchNode }
```

具体约束：

- 每个 patch node 都声明它对应的 shape node；apply 时必须逐层匹配；
- `Replace` 和容器 op 的 payload 在创建或反序列化 patch 时按对应子 shape 验证，内部不得出现 Dynamic payload；
- struct path 使用 field index，apply 同时核对声明中的 field tag，避免 schema 漂移后误写相邻字段；
- enum variant 变化直接使用 `Replace`；variant 相同时才允许 payload 增量；
- `Ref<T>` 默认按 opaque/replace 处理，只有显式策略才递归修改可变引用；
- apply 首先校验 `shape_version` 和 `shape_fingerprint`，成功返回的类型始终为原来的 `T`；
- 网络或磁盘中的 patch 不是可信内部值，必须通过专用的 strict patch decoder 构造。

应满足：

> `apply(shape<T>, old, diff(shape<T>, old, next, strategy)) = Ok(next)`

> `apply(shape<T>, old, patch) = Ok(value)` 蕴含 `value: T`，且保持 struct/enum 名义身份。

### 7.1 核心与 Recollect 的职责边界

| Calcit 核心负责 | Recollect 负责 |
| --- | --- |
| `DataShape<T>` 派生、ABI 与 fingerprint | diff 启发式与遍历调度 |
| typed struct/enum 构造和 indexed 更新 | keyed list（如 `:id`）策略 |
| 不透明 `Patch<T>` 的校验和 apply 原语 | patch 合并、压缩和 memoization |
| Native/JS/WASM 一致的节点语义 | 开放 Dynamic tree 的兼容 API |
| patch strict decoder | 面向应用的 strategy 组合 API |

`DiffStrategy<T>` 绑定到 shape node 或静态路径。Recollect 的 `{:key :id}` 在派生 strategy 时应验证目标节点是 list、元素是 struct/map，且 key 字段存在并具有可比较的闭合类型；不能等运行到某条动态数据才发现策略不适用。

首个内部落地切片已实现 `Keep`、`Replace`、`StructFields` 和 `EnumPayload`：patch 创建和 apply 都核对 node id，替换值通过子 shape 验证，apply 核对 ABI version/fingerprint，并保留 base struct/enum 的名义对象。它暂时不暴露语言 API，也不接受普通 Map/tuple 伪造；下一步才是 dictionary/decoder surface 与 Recollect list/map/set strategy 的接入。

Recollect 的现有 Dynamic API 已先完成一项兼容性收紧：`change-op` 统一用名义 enum 构造，`:map-splice` 的 removed payload 修正为 `Set<Dynamic>`。这能在 operation 产生时检查 variant/arity/container kind，但不把 Dynamic payload 冒充为 `Patch<T>`。

## 8. 与旧 API 的关系

| API | 用途 | 静态结果 | 运行时保证 |
| --- | --- | --- | --- |
| `parse-cirru-edn text [options]` | 任意 EDN、旧代码兼容 | `Dynamic` | 语法有效；options 仅恢复部分身份 |
| `unsafe-coerce value T` | 显式信任外部保证 | `T` / trusted | 不验证、不转换 |
| `assert-type value T` | 现有浅层断言 | 原值 | 仅当前 matcher 覆盖范围 |
| `parse-cirru-edn-as text T` | 严格类型化反序列化 | `T` / validated | 深度结构与名义身份满足 `T` |

旧 API 不自动弃用，因为 REPL、配置浏览、代码数据和开放 EDN 都有合理的动态使用场景。文档应把它称为动态解析，并优先向业务状态恢复推荐严格 API。

## 9. 实施阶段

### Phase 1：严格闭环（已完成）

- 新增 `parse-cirru-edn-as` syntax 和静态返回类型；
- 构建无 Dynamic 节点的 `DataShapeGraph`；
- Native 执行器直接从 `cirru_edn::Edn` 构造类型化值；
- JS codegen 输出等价 graph，JS runtime 执行同样规则；
- 支持有限 struct/enum 组合、泛型替换与 where-bound；
- 增加成功、深层失败、名义名称失败、字段集合失败、variant/arity 失败和编译期拒绝用例；
- 文档明确旧 options map 的能力边界。

### Phase 2：共享 data shape 内核（内部实现已完成）与一等 decoder dictionary

- 已从 decoder 派生逻辑中提炼编译器内部 `DataShapeGraph`，Native/JS strict EDN 共用；
- 已让 typed record assoc/with 校验替换后的字段类型并保持精确接收者类型；
- 引入不可伪造的 `EdnDecoder<T>`；
- 先让命名类型解析和 schema 展开具备 cycle-safe 能力，再开放递归 struct/enum graph；
- `edn-decoder T` 编译期派生并 hoist graph；
- 泛型函数显式接收 `EdnDecoder<T>`；
- 提供 `decode-cirru-edn` 和 `decode-edn` 的 Result 版本；
- 允许显式组合 decoder，但不允许从普通 Map/Record 构造。

### Phase 3：typed diff/patch（内部最小 apply 内核已完成）

- 已实现内部 `DataPatch` 的 `Keep`、`Replace`、`StructFields`、`EnumPayload`；
- 已让 struct 字段和 enum payload patch 绑定到 `DataShapeGraph` node，并校验 index/tag；
- 已让内部 patch apply 校验 ABI/fingerprint、递归结果类型并保持名义身份；
- 引入一等、不透明的 `Patch<T>` 与 `DiffStrategy<T>` surface；
- 增加 list/map/set typed operations 和 strategy；
- 为 patch 的跨进程传输提供同样严格的 decoder，不暴露 Dynamic payload；
- 以 Recollect 的 round-trip 和嵌套 record/map fixtures 验证语义，再决定共享执行器的下沉边界。

### Phase 4：受控的格式演进

- 设计显式 custom decoder / migration API；
- 默认值、旧字段名、版本迁移只能通过 custom 节点引入；
- custom decoder 的结果仍必须进入最终目标类型的已验证构造路径。

## 10. 兼容性与风险

- 新 syntax 不改变旧代码；
- 严格 API 有意拒绝不完整类型，不能为了兼容改成 warning 或 Dynamic fallback；
- 编译期图构建可能增加时间，后续按规范化类型指纹缓存；
- 后续 recursive graph 必须设置构建中占位节点和执行深度/输入规模保护，避免恶意数据造成栈或资源耗尽；
- Native/JS 任一端暂不支持的节点应在编译期统一拒绝，不能只让某后端失败。

## 11. 验收标准

1. `parse-cirru-edn-as` 的推断结果为目标闭合类型；
2. `List<Map<Tag, Person>>` 等嵌套值逐层校验；
3. struct/enum 成功值携带精确声明身份和 impl；
4. Dynamic、缺泛型、未绑定变量、函数/trait/JS object 在运行前被拒绝；
5. Native 与 JS 对相同输入同时成功，或在同一路径以同一错误类别失败；
6. 旧 `parse-cirru-edn` 行为保持兼容，并移除 record 字段不匹配的 panic 路径；
7. 文档与集成测试覆盖动态解析、严格解码和 unsafe coercion 三种边界的区别。
