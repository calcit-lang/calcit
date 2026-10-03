# RFC：trait 的 requires 声明

状态：Partial

日期：2026-10-03

关联：

- [#1535](https://github.com/calcit-lang/calcit/issues/1535)（trait 默认方法与依赖的 RFC，本文只处理其中的 requires）
- [#1704](https://github.com/calcit-lang/calcit/issues/1704) 缺口 3（外部类型之间的子类型关系，本文的第一个消费者）
- [#844](https://github.com/calcit-lang/calcit/issues/844)（按 nominal origin 的严格 trait 分发，已完成）
- `RFCs/09-19-external-object-declaration-shorthand-rfc.md`（`defexternal` 简写）
- `docs/features/traits.md`、`docs/features/js-interop.md`

> 表层写法已审阅（见文末“审阅结论”）。实施步骤 1–3 已落地；步骤 4（js-ffi 事件 trait 迁移与 Respo 回归）在下游仓库进行。

## 摘要

为 `deftrait` 与 `defexternal` 增加一个局部子句 `'requires`，声明“具备本 trait 的值也必须具备另一个 trait”。
它只填充已有的内部字段 `CalcitTrait.requires`，复用已经实现的传递规范化、环检测与按 origin 判等，
不新增类型种类，也不引入隐式的全局实现搜索。

效果分两类：

- **external-object trait**：子 trait 的值可以直接传给要求父 trait 的位置（隐式上转），并能访问父 trait
  声明的字段与方法；`js-cast` 检查全部可达成员。
- **普通 trait**：子 trait 的实现只有在同一类型也挂载了全部父 trait 的实现时才成立；满足子 trait 的约束
  因此可以当作父 trait 的证明使用。

## 现状

内部模型已经具备，只缺声明入口：

| 已有能力 | 位置 |
| --- | --- |
| `CalcitTrait.requires: Vec<Arc<CalcitTrait>>` 字段 | `src/calcit/calcit_trait.rs` |
| `normalized_reachable_traits`：传递展开、按 nominal origin 去重、稳定排序、拒绝环 | 同上 |
| trait 类型之间的证明沿可达集合成立（`trait_annotation_proves`），impl 的 origin 同样沿可达集合匹配（`impl_matches_trait`） | `src/calcit/type_annotation.rs` |
| external-object 字段查找遍历可达 trait | `src/runner/preprocess/mod.rs` |
| `js-cast` 检查可达 trait 的全部成员 | `src/codegen/emit_js.rs`、`src/runner/preprocess/js_ffi.rs` |

但 `&trait::new` 只接收名字与成员列表，没有任何源码写法能让 `requires` 非空，目前只有 Rust 单元测试手动构造。
这也意味着 `impl_matches_trait` 把“子 trait 的实现”当作父 trait 证据的路径从未被真实使用，
普通 trait 侧还缺少对应的实现检查（见“普通 trait”一节），在开放语法前必须补上。

## 消费者案例

js-ffi 的浏览器事件（`js-ffi.browser`）现在把 `KeyboardEventHost`、`MouseEventHost`、`PointerEventHost`
声明为与 `EventHost` 无关的独立 trait：

```cirru.no-check
deftrait EventHost
  :event-type 'String
  :target $ :: 'JsNullish 'JsObject
  .prevent-default! $ :: 'Fn $ {}
    :args $ [] 'js-ffi.browser/EventHost
    :return 'Unit

deftrait KeyboardEventHost
  :key 'String
  :code 'String
  .prevent-default! $ :: 'Fn $ {}
    :args $ [] 'js-ffi.browser/KeyboardEventHost
    :return 'Unit
```

于是每个子事件都要重复声明 `.prevent-default!` 等公共成员；接受任意事件的业务函数只能写成泛型 `'T`
入口再 `unsafe-coerce`（如 `keyboard-event-host`、`mouse-event-from-event`），这正是 #1704 缺口 1 的来源。
拟议写法：

```cirru.no-check
defexternal KeyboardEventHost
  :target :browser
  'requires EventHost
  :key 'String
  :code 'String

defn log-event (event)
  hint-fn $ :: 'Fn $ {} (:args $ [] 'EventHost) (:return 'Unit)
  println $ event :event-type

defn on-key (event)
  hint-fn $ :: 'Fn $ {} (:args $ [] 'KeyboardEventHost) (:return 'Unit)
  log-event event
  event .prevent-default!
```

`log-event` 接受 `KeyboardEventHost` 无需转换；`event .prevent-default!` 解析到 `EventHost` 的声明。
从 `EventHost` 得到 `KeyboardEventHost` 仍需 `js-cast event 'KeyboardEventHost`（检查全部可达成员）。

## 表层写法

`'requires` 是 trait 声明内的一个子句，头部是引用符号 `'requires`（Symbol），值是一个 trait 引用：

```cirru.no-check
deftrait Shape
  'requires Debug
  'requires app.geometry/HasArea
  .draw $ :: 'Fn $ {} (:args $ [] 'Shape) (:return 'Unit)
```

- 每个子句恰好一个 trait，需要多个父 trait 时重复书写；与 `defexternal` 已有的“每个条目一个值”规则一致。
- 成员键只能是 `:field`（Tag）或 `.method`，子句头是 Symbol `'requires`，与 Tag 形式的成员键（包括名为 `:requires` 的宿主字段）
  在类型上区分；旧代码中不存在以 Symbol 开头的成员条目，因此这是纯增量写法。
- trait 引用按普通符号解析（`:require` 导入、命名空间限定），因此天然携带 nominal origin；不接受 tag 或字符串。
- `defexternal` 使用同一子句；规范化后的 Snapshot 存储为 `deftrait` 代码中的同一子句，不放进 `:ffi` 元数据。
- 宏展开为 `&trait::new name members requires-list`，第三个参数可省略，省略时等价于空列表，已有调用不变。

## 语义

### 声明检查

在 trait 求值与预处理阶段（两处使用同一检查）：

1. 每个 `requires` 值必须解析为 trait；否则报错。
2. 可达集合中不得出现环（复用 `normalized_reachable_traits` 的错误文本）；同一父 trait 重复声明按 origin 去重，不报错。
3. external-object trait 只能 require external-object trait，普通 trait 只能 require 普通 trait。二者语义不同（宿主成员形状 vs. 实现挂载），混用没有可解释的含义。
4. 子 trait 自身成员与任一可达父 trait 成员同名时报错，即使签名相同；已有重复声明由 AI 按诊断删除子 trait 中的副本完成迁移。两个父 trait 之间的同名成员同样报错，避免成员访问出现歧义。

### 证明与上转

- 类型为子 trait（含 `Optional` / `JsNullish` 包装）的值满足父 trait 类型的参数、返回值与 `:where` 约束，沿可达集合传递。
- 反方向（父 → 子）永远不隐式成立：external-object 用 `js-cast`，普通 trait 用 `assert-traits` 或模式匹配到具体类型。
- `Dynamic` 与 `JsObject` 不因 requires 获得任何证据。

### 成员可见性

- 对子 trait 类型的接收者，`.method` 与 `:field` 在可达集合中查找；由于声明检查保证不重名，结果唯一。
- 父 trait 方法签名中的接收者类型是父 trait，调用时接收者按上转规则检查，不重写签名。
- `&trait-call` 仍以显式 trait 为准，可以直接指定父 trait。

### 普通 trait 的实现检查

`defimpl ChildImpl Child` 只提供 `Child` 自身的方法，父 trait 的方法由父 trait 自己的实现提供：

- `impl-traits T ...` 挂载任一实现时，检查挂载后的完整实现集合覆盖该实现 origin 的全部可达 trait；缺少父 trait 实现时报错，指出子 trait、缺失的父 trait 与目标类型。
- 只有通过这一检查，`impl_matches_trait` 才能把子 trait 的实现作为父 trait 证据；检查失败的挂载不产生任何证据。
- `assert-traits value Child` 在运行时同样要求父 trait 的实现存在，native 与 JS 一致。
- 不做全局 blanket impl：父 trait 实现必须由用户显式挂载到同一类型。

### 泛型

`:where T Child` 的函数体可以调用 `Child` 与其可达父 trait 的方法；调用方传入的类型须满足 `Child`（因而也满足父 trait）。

### 热重载与查询

- trait 值在求值时捕获父 trait 的值；父 trait 重新定义后，引用它的子 trait 按现有定义依赖重新求值，旧的 `runtime_id` 证据失效，不跨版本混用。
- `calcit query def` 原样显示 `'requires` 子句；`query type` 对子 trait 列出可达父 trait 的成员并标明来源 trait（尚未实现，后续补齐）。

## 诊断

| 情形 | 诊断 |
| --- | --- |
| `'requires` 的值不是 trait、可达集合出现环、external-object 与普通 trait 互相 require、可达成员重名 | `E_TRAIT_REQUIRES`，信息区分具体原因；环沿用现有路径文本 |
| 挂载子 trait 实现但缺少父 trait 实现 | `E_IMPL_MISSING_REQUIRED_TRAIT` |

分两个编号是因为修复位置不同：前者改 trait 声明，后者改 `impl-traits` 挂载。现有 `E_DUPLICATE_TRAIT_IMPL`、
`E_AMBIGUOUS_TRAIT_METHOD`、`E_TRAIT_CALL_IMPL_MISSING` 描述调用期的候选冲突或缺失，不能表达声明期与挂载期错误。

## 正反规格（实现时写成 `:tests` 与 type-fail fixture）

| 用例 | 期望 |
| --- | --- |
| `KeyboardEventHost` 值传给 `EventHost` 参数 | 通过 |
| `KeyboardEventHost` 接收者调用 `.prevent-default!`、读取 `:event-type` | 通过，解析到 `EventHost` |
| `EventHost` 值传给 `KeyboardEventHost` 参数 | 类型错误；`js-cast` 后通过 |
| `js-cast value 'KeyboardEventHost` 缺少父 trait 的宿主成员 | 运行时契约错误，信息含父 trait 成员名 |
| `A 'requires B`、`B 'requires A` | `E_TRAIT_REQUIRES`（环） |
| 两条路径到达同一父 trait（菱形） | 通过，按 origin 去重 |
| 不同命名空间的同名 trait 作为父 trait | 视为不同 trait |
| 子 trait 与父 trait 声明同名成员（含相同签名） | `E_TRAIT_REQUIRES`（成员重名） |
| 普通 trait require external-object trait | `E_TRAIT_REQUIRES`（种类不符） |
| `impl-traits T ChildImpl` 无 `ParentImpl` | `E_IMPL_MISSING_REQUIRED_TRAIT` |
| `impl-traits T ParentImpl ChildImpl` 后 `:where T Parent` 调用 | 通过 |
| `assert-traits value Child`，值只有 `ChildImpl` | 运行时失败（native 与 JS 一致） |
| `:where T Child` 的函数体调用父 trait 方法 | 通过 |
| 重新定义父 trait 后热重载 | 子 trait 重新求值，旧证据不再满足新约束 |

## 后端范围

- native 与 JS：普通 trait 的挂载检查、`assert-traits`、方法分发全部支持。
- external-object trait 仅 JS 后端（与现状相同）。
- WASM：只覆盖能静态确定目标的方法调用；运行时 `assert-traits` 等不支持的路径明确报告 unsupported。

## 兼容与迁移

- 纯增量语法；已有 trait 的 `requires` 为空，行为不变。
- js-ffi 的事件 trait 可在下游 PR 中改为 `'requires EventHost` 并删除重复成员；这会把原来按值拒绝的跨事件传参变为允许，属于放宽，不影响已有调用。重复的泛型 `unsafe-coerce` 入口按 #1704 缺口 1 逐个迁移为 `js-cast`，不批量自动改写。
- 不提供 `calcit fix` 规则：是否建立继承关系由维护者判断。

## 非目标

- trait 默认方法（#1535 的另一半）、associated types、specialization、全局 blanket impl。
- 子 trait 覆盖父 trait 成员签名。
- `Eq` / `Hash` 等 core trait 之间建立 requires（core 能力表不变）。

## 实施步骤（审阅通过后）

1. `deftrait` / `&trait::new` / `defexternal` 规范化接受 `requires`，声明检查与诊断，Snapshot 与 query 往返。
2. 普通 trait 的 `impl-traits` 挂载检查与 `assert-traits` 运行时检查；在此之前不让 `requires` 产生证据。
3. external-object 上转、可达成员访问与方法分发；`js-cast` 回放测试。
4. js-ffi 事件 trait 迁移并用 Respo 回归（下游 PR）。

## 审阅结论

1. 子句头使用 Symbol `'requires`，与 Tag 成员键区分。
2. 子 trait 与父 trait 同名成员一律报错，已有重复由 AI 按诊断迁移。
3. 声明期错误合并为 `E_TRAIT_REQUIRES`，挂载期保留 `E_IMPL_MISSING_REQUIRED_TRAIT`。
