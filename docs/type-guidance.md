---
title: "Type Guidance"
summary: "Dynamic 审计、Option/Result 组合、嵌套数据访问和类型化 Enum 构造"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "dynamic audit"
  - "option result"
  - "typed enum"
id: core/features/type-guidance
parent: core/features
---

# Calcit 类型使用指南

Calcit 的类型规则处于表层语言与 macro 展开后的 typed core 之间，native、JS 和 Calx 复用同一份已经建立的
类型关系。完整的分层 owner、开放值与 Agent source-fix 契约见
[`09-12-layered-semantics-and-agent-fixes-rfc.md`](../RFCs/09-12-layered-semantics-and-agent-fixes-rfc.md)。

## Dynamic 是边界，不是默认多态

`Dynamic` 适合 JS FFI、框架开放数据、宏和确实无法提前知道的外部输入。普通函数不要用多个 `Dynamic` 表示“它们应该是同一个类型”：输入和返回关联时用 `:generics` 与 TypeVar；只需要能力时用 trait 与 `:where`；同质集合写出元素类型；有限异构数据定义为 Enum；可缺失值使用 `Option<T>`，带失败信息使用 `Result<T, E>`。

Dynamic 表示用户明确选择的开放 Calcit 值，不等于编译器尚未推断出的 Unknown/Unresolved，也不等于宿主拥有的
`JsObject`。开放值可以被保存、传递、放入开放容器或包装进 `Option<Dynamic>`；只有将其当成 Number、String、
具体 Struct/Enum 或某个 trait 能力使用时，才需要 decode、narrow 或显式 unsafe boundary。真正参数化且不观察
内容的泛型函数可以携带 Dynamic；声称具体内容关系但没有证据的调用继续拒绝。

普通执行、编译和严格检查只报告可执行的 warning/error，不计算 Dynamic 比例。迁移存量代码时再显式查询具体位置：

```bash
calcit analyze check-types --summary-only
calcit analyze weak-types --only schema-dynamic,unresolved-type-slot,code-dynamic --intent unresolved --format json
```

## Schema 类型别名在各后端一致展开

非 Dynamic definition schema 可以通过完整的 `'namespace/definition` 名称作为类型别名使用。别名只复用底层类型关系，不创建新的运行时包装；需要名义身份时仍应使用 `defstruct` 或 `defenum`。

```bash
calcit edit def app.schema/Items --code 'quote $ def Items &unit'
calcit edit schema app.schema/Items --code "quote \$ :: 'List 'Number"
```

此后 `'app.schema/Items` 可用于 Struct 字段、Enum payload、callback 或 trait 边界。静态检查、Native 构造验证和 JavaScript codegen 都按同一底层 schema 判断值；不要为 Native 单独增加 coercion。类型别名不能接收泛型实参，循环别名也不会退化成 Dynamic，而是作为无法证明的类型关系拒绝。

兼容性的多态 collection facade 可能仍在 core schema 中保留局部 `Dynamic`，或者使用彼此独立、无法表达容器成员关系的泛型，但已知 receiver 会在预处理阶段专门化。例如 `update` 对 `List<T>` 要求 `Number` 索引和 `T -> T` updater，对 `Map<K,V>` 要求 `K` 键和 `V -> V` updater；Struct 则按静态字段类型检查。`filter`、`any?` 与 `every?` 对 List/Set 要求 `T -> Bool` predicate，`each` 则约束 callback 输入为 `T`、允许任意返回类型；`map` 对 List/Set 要求 `T -> U` mapper，并把 Set receiver lowering 到 `&set:map`。`foldl` 与 `reduce` 从初始值恢复 accumulator `U`，并要求 reducer 为 `U, T -> U`；原生 `foldl` 只有在 reducer 具有具体且兼容的 `Fn` 签名时才把初始 accumulator 类型保留为返回类型，`DynFn` 仍推断为 `Dynamic`。普通 `apply f args` 只会在 `args` 是非 Dynamic 的同质 `List<T>`、`T` 能满足 `f` 的全部 fixed/rest 输入、且展开长度能证明 callable arity 时恢复 `f` 的具体或泛型返回类型；若参数位置异构、固定参数调用的 list 长度未知、callable 未知，或存在 trait-bounded 泛型，则兼容返回仍为 `Dynamic`，应改为直接调用、先归一化参数，或在审核过的开放边界显式保留 Dynamic。双参数 `sort` 与 `&list:sort` 保留 `List<T>`，并要求 comparator 为 `T, T -> Number`；函数形式的 `&list:sort-by` 要求 selector 为 `T -> K`，同时保留 Tag 字段选择器兼容路径。List 的 `.apply` 要求函数列表中的每一项共享 `T -> U` 契约，并返回 `List<U>`；它的 direct/method 诊断会用 receiver/input 已绑定的 `T` 显示具体 callback 类型，异构输入或函数列表必须先归一化或拆成多次调用。`interleave` 同样只接受两份 `List<T>` 并返回 `List<T>`；异构数据必须先归一化，或在经过审核的开放边界显式声明 `List<Dynamic>`。单参数自然排序不受影响，Syntax collection 继续使用 phase-aware 开放契约。Map callback 接收运行时的异构 `[key value]` pair；迭代、predicate 与 fold 输入只承诺 `List<Dynamic>`，而 `map` 同时要求 callback 返回另一个 `List<Dynamic>` pair，不会把不同的 `K` / `V` 伪装成同一种成员类型。旧 `map-kv` 还允许 nil/任意 Enum 作为 drop sentinel，因此返回只能是显式兼容边界：兼容模式警告，strict 模式拒绝。typed code 应统一改用 `filter-map-kv`，以 `MapEntryDecision :keep key value` 或 `:drop` 让输出 key/value 与 callback payload 保持可证明关联。`get` 同样要求 List/String/Enum 的 `Number` 索引或 Map 的 `K` 键，`includes?` 要求 List/Set 的成员 `T`、Map 的值 `V` 或 String substring；`contains?` 要求 List/String/Enum 的 `Number` 索引、Map 的键 `K` 或 Set 的成员 `T`。`assoc` 会同时约束 List 的索引/成员、Map 的键/值、静态 Struct 字段的值类型，以及 Enum 的 `Number` payload index；Enum payload 可以异构，因此新值在没有精确 variant/slot evidence 时仍保持开放。`dissoc` 会检查全部 rest 参数：List 只能接收 `Number` 索引，Map 的每个键都必须是 `K`。用户函数 schema 的 `:rest` 会逐项检查。原生 proc 按运行时契约区分两类 typed variadic：`&map:dissoc`、`&list:concat` 与 `&merge` 会检查每一个 rest 参数，并在容器不匹配时显示完整成员类型；`[]` 与 `#{}` 的 `Variadic<T>` 只用于推断公共成员类型，异构字面量仍有意回退为 `Dynamic`。未标注的 inline callback 若能从函数体恢复返回类型，也会参与这项检查。不要把 receiver 擦除为 `Dynamic` 来绕过这些关系：在 FFI/open-data adapter 中先校验或转换，再进入集合操作。

对已知 receiver 的集合调用，成员类型会先进入 inline callback 的参数与函数体，再由同一份 contract 检查调用参数和推断返回值。`any?`、`every?` 与 `each` 因而不需要让 callback 暂存未实例化的泛型，也不需要为使用它们的宏增加例外。

## 严格检查与迁移期 quality baseline

新项目以默认严格预处理和实际目标测试作为发布门禁：

```bash
calcit calcit.cirru --check-only
calcit calcit.cirru --entry test
```

`analyze check-types` 与 `analyze weak-types` 只帮助定位迁移清单，不决定程序是否类型正确，也不输出 Dynamic 比例、shape/family 排名或另一套关系判断。需要清理时使用 kind、intent、definition、path 和 detail 回到源码；值能否进入 typed code 只由严格预处理诊断决定。

已有 CI 的 `analyze quality` 与 baseline 在 0.14.x 保留为有界兼容面，便于存量项目逐步把债务清零：

```bash
calcit calcit.cirru analyze quality --baseline config/calcit-quality.cirru
```

兼容期不要为新项目生成 baseline，也不要增加 metric、intent 或统计协议。每次迁移应降低已有预算，并同时保证默认严格检查通过；清零后从 CI 删除 baseline 命令和文件。0.15 将不再把 coverage/Dynamic 数量当作独立的类型正确性策略，具体删除范围以届时 release migration note 为准。

baseline 是已提交的 Cirru EDN 机器生成工件。为使 GitHub 语言统计忽略其行数，同时保留文本 diff，
可在项目根目录的 `.gitattributes` 加入生成物标记：

```gitattributes
config/*-quality.cirru text linguist-generated=true
```

这不会忽略或删除 baseline；文件仍保留文本 diff，但不计入语言统计。只有外部工具明确要求 JSON
时才使用 `.json` 输出路径。更新 baseline 的 PR 仍应按 definition 审阅预算变化。

## Option / Result 组合

优先让 `Option` / `Result` 的方法表达类型流，而不是逐层 `unwrap` 或调用
`option:*` / `result:*` 的函数形式：

```cirru.no-check
user .and-then $ fn (user)
  (get user :profile) .and-then $ fn (profile) (get profile :name)

loaded .and-then $ fn (value) (validate value)
```

备用来源使用 `.or-else`。`.unwrap-or` 只用于确实需要默认值的终点，`.map` 用于同步转换，`.and-then` 用于下一个仍可能失败的操作。保留 `Option` 本身能让类型系统持续检查缺失路径；不要为了集合判断而把它解成 `nil`。

多个连续的 Option 步骤可以实验性使用 `option:let`。Result 流程直接使用接收者
`.and-then`，让错误类型的转换保持可见：

```cirru.no-check
let
    source $ fs:path |data.cirru
    content-result source.read-text
  content-result.and-then $ fn (content)
    (parse-data content) .and-then $ fn (data)
      save-data data
```

先用 `fs:path` 把 UTF-8 String 提升为 nominal `FsPath`，再调用 `.read-text`、
`.read-dir`、`.walk-dir` 或 `.write-text`，这些方法返回 `Result<...,String>`。
String 本身不携带文件系统语义；`try-read-file`、`try-read-dir`、
`try-write-file` 与底层 raising procedures 仅保留为兼容入口。
这些文件效果支持 native 与生成的 JavaScript；WASM 尚未提供宿主文件效果。

Native 异步 FFI capability 也遵循相同边界原则。模块适配层用 `ffi:task`、
`ffi:response` 把不透明 AnyRef 提升为 nominal `FfiTask`、`FfiResponse`，业务层调用
`.cancel`、`.cancel-with`、`.resolve`、`.reject`。raw 字段保持 `Dynamic`，但
reason/payload 使用方法级泛型，因此不会为了宿主编码而抹掉调用侧类型。底层
`&ffi-task-cancel`、`&ffi-response-*` 只作为适配与兼容入口。

`option:let` 的 body 必须继续返回 `Option`。普通组合函数仍以接收者方法
作为公开形式，`option:*` / `result:*` 直接函数调用主要保留给 core lowering。

## get-in / assoc-in / update-in

`get-in` 是可能失败的开放数据访问，返回 `Option<T>`。当接收者是完整类型的嵌套 Map、路径是非空字面量时，编译器会把 `get-in`、`assoc-in` 和 `update-in` 展开为类型化的直接 `get` / `assoc` 链，不再经过运行时动态路径遍历；调用参数仍按源码顺序各求值一次。`update-in` 的 updater 接收 `Option<T>`，因此缺失叶子不会退回 nil。

动态接收者、动态路径和混合容器仍是明确的兼容边界，会保留动态路径 API。新代码在数据形状已知时优先使用直接 `.get`、Option 组合和名义字段访问；只有开放数据才使用路径 API。不要用路径函数绕过 Struct 字段检查：Struct 应使用 `(:field value)` 或 `value.:field`，字段可缺失就把字段声明为 `Option<T>`。

`Map<K,V>` receiver 的 `.get`，以及 `List<T>`、String、Enum receiver 的 `.get`、`.nth`、`.first`、`.last`，会在 preprocess 阶段降低为对应类型的 `&scope:*` primitive 与显式 Option 构造，不再执行通用函数中的 receiver type predicates。对应的前缀形式保持兼容并使用同一 lowering；业务代码优先使用 receiver 形式来保留类型意图。

对 `update-in` 的缺失值给默认值或明确处理 `%none`，不要无条件 unwrap：

```cirru.no-check
update-in data ([] :settings :retries)
  fn (current) (current .unwrap-or 0)
```

## Enum 构造

Struct 也支持同样的类型化头部调用：

```cirru.no-check
defstruct Profile (:name 'String)
  :bio $ :: 'Option 'String

Profile :name |Ada
```

参数必须是 `:field value` 对，必填字段不能省略；末尾声明为 `Option<T>`
的字段可以省略，Calcit 会补成 `%none`。需要显式控制所有字段时使用
`%{} Profile ...` 并完整提供字段。`%{}?` 会把遗漏字段补成 `nil`，只作为迁移期
兼容入口保留，并会被默认严格诊断以 `E_PARTIAL_STRUCT_NIL_FILL` 拒绝；迁移期只能用 `--compat-types` 暂时恢复旧行为。

已知 Enum 定义时使用头部调用：

```cirru.no-check
Option :some value

Result :err message
```

Calcit 会根据 Enum 定义检查 variant 和 payload，并在预处理阶段生成命名构造。`%::` 保留给显式 prototype、动态跨模块构造和兼容旧代码的边界。
