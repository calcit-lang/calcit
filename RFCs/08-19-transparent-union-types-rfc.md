# RFC: `deftype` 具名透明联合类型与控制流收窄

状态：Partial（MVP 第 1 轮已实现）
日期：2026-08-19
关联：`02-04-runtime-traits-plan.md`、`04-15-match-syntax-rfc.md`、`05-31-generic-where-bounds-mfs.md`、`06-01-generic-binding-unification-rfc.md`、`07-26-static-semantic-analysis-rfc.md`、`08-18-calcit-typed-js-ffi-boundary-rfc.md`

## 1. 摘要

本 RFC 提议新增 `deftype`，用于声明**具名、透明、无运行时包装**的联合类型，并补齐围绕联合类型的控制流收窄能力。

首要用例是 Respo 虚拟 DOM：`Component` 与 `Element` 是两个不同的 Struct，但 `tree`、`children`、diff 与 effect 遍历都需要把它们当作同一类节点传递。当前只能把这些位置声明成 `Dynamic`，或使用 `defenum` 再包一层 constructor。前者失去字段安全，后者改变数据表示并给 DSL 增加构造、解包噪音。

目标写法：

```cirru.no-check
deftype RespoNode
  or 'Component 'Element
```

`Component` 和 `Element` 的值可以直接进入 `RespoNode` 位置，运行时仍保持原有 Struct 表示。代码通过 `struct-match`、`type-match?` 或带 `:narrows` 契约的 predicate 恢复具体成员类型。

### 当前实施范围（2026-08-19）

已落地第一轮 MVP：`deftype Name (or ...)` 是编译期透明声明，并在声明点拒绝无效成员；裸 `TypeRef` 在赋值检查和 Struct 字段运行时验证处展开为成员集合；现有 `&struct:matches?` 的 true branch 和 `struct-match` binder 会获得匹配 Struct 的具体类型。`struct-match` 对 transparent union 检查非成员/重复分支与覆盖完整性。运行时没有 union wrapper，也没有新的 JS ABI。

尚未实现的部分保持为本 RFC 的后续阶段：`_` branch 的剩余 union、公共 `type-match?`、用户 `:narrows` guard、参数化 `deftype`，以及 data-shape/严格 EDN decode 对 union 的支持。

## 2. 设计判断

### 2.1 `deftype` 与 `defenum` 分工

`defenum` 继续表示需要运行时 tag、payload arity 和构造器身份的代数数据类型：

```cirru.no-check
defenum RequestState
  (:idle)
  (:loading)
  (:failed 'String)
```

`deftype` 表示已有类型之间的静态集合，不产生新的值构造器：

```cirru.no-check
deftype RespoNode
  or 'Component 'Element
```

两者分别对应两种不同需求：

- `defenum`：值本身需要携带“属于哪个 variant”的新表示；
- `deftype`：值已经有可靠的运行时身份，只需要描述“这个位置允许哪些类型”。

### 2.2 `deftype` 与 trait 分工

Trait 描述开放的能力集合，适合算法只依赖共同方法的场景；联合类型描述封闭的数据备选，适合分支后读取各自字段的场景。

Respo renderer 需要区分 `Component` 与 `Element`，并读取完全不同的字段，因此核心节点应使用联合类型。DOM FFI 只关心对象支持哪些字段和方法，应继续使用 `:kind :external-object` trait。

若联合类型的所有成员都实现同一个 trait，联合值可以满足该 trait bound；这不把 union 自动转换成新的 runtime trait object。

### 2.3 与 Rust、MoonBit 经验的关系

本设计沿用 Rust/MoonBit 的两条经验：

1. 封闭的数据分支应由编译器进行穷尽性与分支类型检查；
2. 开放扩展和行为分派交给 trait/interface，不用一套机制同时承担两种职责。

Calcit 的差异是保留动态语言的数据表示：`deftype` 不要求像 Rust/MoonBit enum 一样重新包装已有值。它更接近一个具名的静态 sum，但每个成员仍使用自身的 nominal runtime identity。

## 3. 语法

### 3.1 基本形式

`deftype` 接收名称和一个类型表达式。MVP 只开放 `or` 类型表达式：

```cirru.no-check
deftype RespoNode
  or 'Component 'Element

deftype AttrValue
  or 'String 'Number 'Bool 'EventHandler
```

这里采用普通前缀语法，没有引入 `A | B` 之类的中缀 token。对应 AST 形状稳定为：

```json
["deftype", "RespoNode", ["or", "'Component", "'Element"]]
```

较长的声明可以使用 Cirru 的 `,` splice 保持同一个 `or` 表达式：

```cirru.no-check
deftype DomPropValue $ or
  , 'String
  , 'Number
  , 'Bool
  , 'EventHandler
```

推荐短 union 保持单行 RHS；只有成员较多时使用上面的展开写法。

### 3.2 类型引用

其他 schema 使用普通 nominal 引用，不展开成员：

```cirru.no-check
defstruct Component (:name 'Tag)
  :tree $ :: 'Optional 'RespoNode

defn render-node (node)
  struct-match node
    Component component
      :tree component
    Element element
      :children element

:: 'Fn $ {}
  :args $ [] 'RespoNode
  :return 'Unit
```

`RespoNode` 在 public schema、诊断和类型自省中保留名字；只有匹配和归一化时才读取成员集合。

### 3.3 泛型边界

MVP 不开放参数化 `deftype`，避免同时引入 alias 参数替换、递归 kind 检查和高阶类型问题。以下能力留作独立扩展：

```cirru.no-check
; Future, not part of MVP.
deftype ScalarOr (T)
  or 'T 'String 'Number
```

现有参数化 `defenum`、Struct 和容器类型不受影响。

## 4. 静态语义

### 4.1 名义名称，透明成员

`RespoNode` 是具名定义，工具和 schema 不应在输出中随意展开为匿名集合。但赋值关系按照成员透明计算：

- `Component` 可以赋给 `RespoNode`；
- `Element` 可以赋给 `RespoNode`；
- `RespoNode` 不能在未收窄时赋给 `Component`；
- union `A` 可以赋给 union `B`，当且仅当 `A` 的每个成员都能赋给 `B`；
- union 与成员的匹配必须保持方向性，不能因为其中一侧是宽类型而双向通过。

这与集合包含关系一致：实际值的可能集合必须是目标类型允许集合的子集。

### 4.2 归一化

解析 `or` 时执行：

1. 解析 namespace-qualified TypeRef；
2. 展平嵌套 union；
3. 按 nominal identity 去重；
4. 拒绝零成员和单成员 union，单类型别名不属于 MVP；
5. 拒绝直接或间接只由 alias 构成的循环；
6. 允许通过 Struct/Enum 字段形成递归数据图；
7. union 出现 `Dynamic` 时给出错误，因为 `or Dynamic T` 等价于丢失整个约束。

例如 `Component.tree -> Optional<RespoNode>` 与 `RespoNode -> Component | Element` 是合法递归；`deftype A (or 'B)`、`deftype B (or 'A)` 不是。

### 4.3 构造与返回

`deftype` 不生成 constructor，也不改变 `%{}`、`%::` 或字面量：

```cirru.no-check
let
    component $ %{} Component (:name :root)
    element $ %{} Element (:name :div)
    nodes $ [] component element
  render-all nodes
```

当 `render-all` 的参数声明为 `List<RespoNode>` 时，list literal 与 `conj`/`append` 的泛型统一应允许成员提升到 union。

MVP 不从任意不同分支自动合成匿名 union。只有存在以下证据时才提升到具名 union：

- 函数参数或返回 schema；
- 容器的期望元素类型；
- `assert-type`；
- 已有 local 类型；
- 明确的 `deftype` 定义引用。

这样避免一次普通 `if` 把整个程序推断成不断增长的匿名类型集合。

### 4.4 未收窄操作

union 值未收窄前只能执行所有成员都安全支持的操作：

- 可以传给接受该 union 或更宽 union 的函数；
- 可以执行所有成员共同满足的 trait bound/method；
- 不允许直接读取某个成员独有的 Struct 字段；
- 不因为多个 Struct 恰好有同名字段就默认进行 structural field merge。

最后一条是有意限制。共同字段合并会引入字段 variance、optional 与写操作规则，MVP 先要求显式收窄。

## 5. 控制流收窄

仅有 union 声明不足以替代 `Dynamic`。必须让运行时判定产生静态证据，而且证据要能通过 `if`、`cond`、`and` 和 pattern matching 传播。

### 5.1 `struct-match`

Phase 1 直接增强现有 `struct-match`，不新增 Respo 专用 accessor：

```cirru.no-check
defn node-name (node)
  struct-match node
    Component component
      :name component
    Element element
      :name element
```

若 scrutinee 是 `RespoNode`：

- `Component` 分支 binder 类型为 `Component`；
- `Element` 分支 binder 类型为 `Element`；
- pattern 必须是 union 的 Struct 成员；
- 所有成员已覆盖时不要求 `_`；
- 缺少成员且没有 `_` 时给出穷尽性诊断；
- `_` binder 保留尚未覆盖成员组成的剩余 union，而不是退化成 `Dynamic`。

这项能力应修复当前 `struct-match` runtime 能匹配、但分支 binder 仍缺少具体静态类型的问题。

### 5.2 `type-match?`

新增公共 predicate `type-match?`，参数顺序保持 value-first：

```cirru.no-check
if
  type-match? node Component
  :tree node
  nil
```

它同时承担运行时判定与编译器可识别的 narrowing primitive：

- true branch：`node` 收窄到 `Component`；
- false branch：从原 union 排除 `Component`；
- 第二个参数必须是静态可解析的具体类型定义；
- external-object trait 只有静态证据，没有可靠 runtime identity，不允许用于该 predicate；
- 参数化容器只检查外层 runtime kind，不声称验证内部元素类型。

底层可复用现有 `&struct:matches?`、Enum definition identity 和 builtin kind 判定，但业务代码不应直接依赖这些 primitive 的组合。

### 5.3 用户定义 type guard

为了保留 `component?`、`element?` 这类领域名称，函数 schema 新增 `:narrows`：

```cirru.no-check
defn component? (value)
  type-match? value Component

:: 'Fn $ {}
  :args $ [] 'RespoNode
  :return 'Bool
  :narrows $ {}
    0 'Component
```

key 是从零开始的参数位置，value 是 true 分支证明的目标类型。规则如下：

- 被标记函数必须返回 `Bool`；
- 目标类型必须是对应参数声明类型的成员或子类型；
- 编译器必须验证函数体是可证明等价的 guard；MVP 不提供绕过验证的 trusted 标记；
- 普通函数不能仅靠 schema 谎称 narrowing；
- false 分支从原 union 排除目标类型。

MVP 只接受函数体直接调用 `type-match?` 的可验证 guard。组合 guard 和用户自定义验证器留到后续，避免把 `:narrows` 变成另一种 `unsafe-coerce`。

### 5.4 逻辑表达式传播

`and` 必须按从左到右的短路语义传播 true evidence：

```cirru.no-check
if
  and
    component? old-tree
    component? new-tree
  compare-components old-tree new-tree
  nil
```

调用 `compare-components` 时，两个 local 都是 `Component`。`or` 的 true 分支通常只得到多个可能性的 union，false 分支则累积排除证据。

`cond` 每个后续分支继承前面 predicate 为 false 的排除结果：

```cirru.no-check
cond
    component? node
    render-component node
  (element? node)
    render-element node
```

当 `node` 是 `Component | Element` 时，第二个 condition 进入前已经排除了 `Component`；`element?` 再确认具体类型。实现应保存每个 local 的 positive/negative type set，而不是只记录单个覆盖类型。

## 6. 通用类型匹配

`struct-match` 足以覆盖 Respo 的第一阶段。为了让 union 能包含 Struct、Enum 和 builtin 类型，后续增加原生 `match-type`：

```cirru.no-check
match-type value
  Component component
    :tree component
  Element element
    :children element
  String text
    count text
```

每个 arm 是 `Type binder body...`，符合 Cirru 现有缩进结构，不需要把 pattern 和 body 包进多层括号。

`match-type` 的职责是按具体 runtime type identity 分支；Enum 内部 variant 仍交给现有 `match`。例如先由 `match-type` 确认值属于某个 Enum definition，再在分支内用 `match` 解构 variant。

Phase 1 不要求实现 `match-type`；但 `deftype` 的 IR 与 exhaustiveness API 不应锁死为 Struct-only。

## 7. 与 Optional、Option 和 nil 的关系

union 不隐式包含 `nil`。缺失值继续由现有类型表达：

```cirru.no-check
defstruct Component (:name 'Tag)
  :tree $ :: 'Optional 'RespoNode
```

迁移期可以使用 `Optional<RespoNode>` 保持现有 nil 表示。新 API 若要显式表达业务分支，仍优先使用 `Option<RespoNode>`。

不建议声明 `RespoNode = Component | Element | Unit` 来偷渡 nullable 语义，因为这会把“没有节点”和“函数无返回值”混在一起。

## 8. 与 trait 和 FFI 的边界

### 8.1 普通 runtime trait

若 union 所有成员 nominally implement `RenderNode`，则：

- `RespoNode` 可以满足 `T: RenderNode`；
- `.method` 继续根据实际 Struct/Enum 的 impl 分派；
- 任一成员缺少 impl 时，整个 union 不满足该 trait；
- 多成员同名 inherent method 不构成 trait 证据。

这使 union 与 runtime trait 互补，而不是竞争两套多态模型。

### 8.2 external-object trait

DOM FFI 继续直接返回小型 external trait，例如 `DomElement`、`DomInput`、`DomKeyboardEvent`。不要用 union 模拟 DOM interface inheritance，也不要把宿主对象与 Respo 内部节点放进同一个 union。

External trait 是 codegen-only 静态证据，不能参与 `type-match?` runtime narrowing。宿主 API 的字符串相关重载，例如 `keydown -> KeyboardEvent`，优先通过专用 wrapper 表达，不在 union 系统中增加 dependent typing。

## 9. 表示与实现建议

### 9.1 类型表示

建议新增两层表示：

- `Calcit::TypeUnion` 或等价 definition value：保存名称、namespace、RHS 和 nominal identity；
- `CalcitTypeAnnotation::UnionRef`：保留具名引用以及按需解析后的规范化成员。

不要在普通值上增加 `Calcit::UnionValue`。`Component` 值仍然是 `Calcit::Struct`，`Element` 值也仍然是 `Calcit::Struct`。

`type-of RespoNode` 可返回 `:type-def`，`type-of component` 仍返回 `:struct`。类型自省后续可增加 `&type:members`；MVP 只需要编译器内部 lookup。

### 9.2 解析与生命周期

`deftype` 应与 `defstruct`、`defenum` 一样保持 top-level definition identity，并参与 snapshot/schema 解析。RHS 解析必须延迟到 namespace definitions 可见后，支持 Struct 字段与 union 之间的递归引用。

JS/native/WASM 均不需要为 union value 新增 ABI。主要后端工作是：

- 保留或擦除 type definition metadata；
- lower `type-match?` 与 `match-type`；
- 在 codegen 前完成字段访问和 method candidate 校验。

### 9.3 类型匹配实现

建议把 assignability 写成方向明确的关系：

```text
is_assignable(actual, expected)
```

核心 union 规则：

```text
member M -> union U      iff M -> any member of U
union A  -> union B      iff every member of A -> B
union U  -> non-union T  iff every member of U -> T
```

最后一条通常只有 union 归一化后所有成员都可赋给同一个 trait/宽类型时成立，不能作为 downcast。

现有 `Dynamic` 兼容规则不能用来证明 union member。若 strict schema 中 `Dynamic` 与具体成员双向匹配，联合类型仍会退化成无约束；这部分应与静态语义 RFC 一起改成方向性边界规则。

## 10. 诊断

建议新增稳定诊断码：

| Code | 条件 | 建议 |
| --- | --- | --- |
| `E_UNION_EMPTY` | `or` 没有成员 | 至少声明两个具体类型 |
| `E_UNION_SINGLE_MEMBER` | MVP 中只有一个成员 | 直接使用该类型 |
| `E_UNION_DYNAMIC_MEMBER` | union 包含 `Dynamic` | 移除 `Dynamic` 或保留整个位置为显式动态边界 |
| `E_UNION_ALIAS_CYCLE` | alias-only 循环 | 通过 Struct/Enum 字段建立递归 |
| `W_UNION_REQUIRES_NARROWING` | 对 union 读取成员独有字段 | 使用 `struct-match`、`type-match?` 或可信 guard |
| `W_UNION_NON_EXHAUSTIVE` | match 缺少成员 | 补齐 arm 或 `_` |
| `W_INVALID_NARROWS_CONTRACT` | `:narrows` 与函数体/参数不一致 | 改成直接 `type-match?` guard |

诊断应显示 union 名称和剩余成员，例如：

```text
W_UNION_REQUIRES_NARROWING: `RespoNode` may be Component | Element;
field `:tree` only exists on Component. Narrow `node` before access.
```

## 11. Respo 迁移目标

第一轮迁移只改类型关系，不重写 renderer 架构：

1. 声明 `RespoNode = Component | Element`；
2. 把 `Component.tree`、renderer/diff/effect 参数改为 `RespoNode` 或 `Optional<RespoNode>`；
3. 增强 `struct-match` binder narrowing；
4. 给 `component?`、`element?` 增加可验证 `:narrows`；
5. 删除 `as-component`、`as-element` 以及对应的 `&struct:nth` accessor；
6. 再分别为属性值、style 值、coord key 和事件值声明小型 union；
7. DOM host object 保持 external trait，不与 `RespoNode` union 混用。

预期 diff 主干可以保持当前数据导向写法：

```cirru.no-check
cond
    and
      component? old-tree
      component? new-tree
    diff-components old-tree new-tree
  (and (element? old-tree) (element? new-tree))
    diff-elements old-tree new-tree
```

这里不需要 runtime enum wrapper，也不需要把整个 diff 算法改写成 trait virtual methods。

## 12. 实施阶段

### Phase A：`deftype` 与 assignability

- parser/snapshot 能加载 `deftype Name (or ...)`；
- 新增具名 union annotation 与 definition lookup；
- 实现成员到 union、union 到 union 的方向性匹配；
- 支持 Fn 参数/返回、Struct 字段和容器 expected type；
- `check-types`、`analyze weak-types` 和类型打印保留 union 名称。

### Phase B：`struct-match` 静态收窄

- branch binder 得到具体 Struct 类型；
- 对 union 做成员合法性和穷尽性检查；
- `_` 分支获得剩余 union；
- 字段读取正常 lower 到受检 `&struct:nth`。

完成 A+B 后，Respo 已可移除大部分 `Dynamic -> Component/Element` adapter。

### Phase C：predicate 与 flow facts

- 实现 `type-match?`；
- 实现可验证 `:narrows`；
- 在 `if`、`cond`、`and`、`or` 中传播 positive/negative member sets；
- 让多个 local 的 evidence 可以同时存在。

### Phase D：通用匹配与共同能力

- 实现 `match-type`；
- union 对共同 trait bound 的满足检查；
- 按真实项目需要评估共同只读字段，不默认开放；
- 再评估参数化 `deftype` 与匿名 union inference。

## 13. 验收标准

至少覆盖以下测试：

1. `Component`、`Element` 都能传入 `RespoNode` 参数；
2. 其他 Struct 不能传入；
3. `List<RespoNode>` 可构造异构列表，元素读取仍是 `RespoNode`；
4. 未收窄访问 `:tree` 给出稳定诊断；
5. `struct-match` 两个 branch binder 分别拥有具体 Struct 类型；
6. 完整分支无穷尽性告警，缺少分支会告警；
7. `component?` true/false 分支分别保留包含/排除证据；
8. `and` 能同时收窄 old/new 两个 local；
9. `or Dynamic Component` 被拒绝；
10. union 值运行时表示、相等性、hash 和序列化与原成员完全一致；
11. native 与 JS 的 `type-match?` 结果一致；
12. external-object trait 不被误当成可运行时判定的 union member。

## 14. 不采用的方案

### 14.1 保持 `Dynamic`，依靠 accessor

这会让 `as-component` 和 `&struct:nth` 只隐藏类型缺失，不提供运行时验证，也无法让容器、返回值和递归字段形成稳定关系。

### 14.2 只使用 runtime trait object

Trait 适合共同方法，但 Respo diff 需要按具体节点种类读取不同字段，并同时比较 old/new 两个值。把全部逻辑改成 virtual methods 会引入大量 accessor 或双分派，复杂度高于封闭 union。

### 14.3 使用 `defenum` 包装已有 Struct

该方案类型安全，但每个节点都要额外构造和解包：

```cirru.no-check
defenum RespoNode
  (:component 'Component)
  (:element 'Element)
```

这会改变 Respo 当前数据表示、相等性路径和 DSL 输出。`deftype` 的目标正是获得相同的静态分支能力，而不引入这层运行时包装。

### 14.4 自动把不同分支推断为匿名 union

全局自动合成会让 union 随控制流不断增长，并使错误信息缺少稳定领域名称。MVP 只在具名 expected type 已知时提升。

## 15. 结论

要在不依赖 `Dynamic` 的前提下保持 Calcit/Respo 的数据导向风格，`deftype` 是基础能力，flow narrowing 是不可分割的另一半。只实现声明而不实现 `struct-match` binder、predicate guard 和逻辑传播，最终仍会回到手写 cast/accessor。

建议按 A+B 先打通 RespoNode，再实现 C；`match-type`、参数化 alias 和更积极的推断放到真实迁移数据证明有必要之后。
