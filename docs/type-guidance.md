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

Calcit 的类型规则处于表层语言与 macro 展开后的 typed core 之间，native、JS 和 WASM 复用同一份已经建立的
类型关系。完整的分层 owner、开放值与 Agent source-fix 契约见
[`09-12-layered-semantics-and-agent-fixes-rfc.md`](../RFCs/09-12-layered-semantics-and-agent-fixes-rfc.md)。

## 可省略声明的闭合 helper

普通、非递归、零参数 `defn`，在函数体能够证明完整返回类型时，可以省略重复的 root `:schema`。例如下列 helper 可由正常预处理得到 `Fn () -> Number`：

```cirru
defn initial-count () $ .count $ [] 1 2 3
```

调用这个 helper 的普通方法和回调继续读取同一份已检查签名；不是从某一个调用、测试或示例猜测公开契约。编译器只在 typed core 中补充签名，不修改 Snapshot。已有明确 schema 仍然优先；显式 `Dynamic` 不会被当成缺省声明自动收紧。

当前这一步不推断有参数 helper 的公开签名，也不放宽递归、WASM import/export、FFI 元数据和异步边界。函数体存在冲突分支、未确定的类型参数或开放容器时，仍需补足契约。省略声明不是关闭类型检查，错误使用推断结果仍会失败。

Agent 可用现有查询核对证据：

```bash
calcit query type-at app.main/initial-count --path code --format edn
calcit query context app.main/initial-count --format edn
```

`type-at` 返回完整推断函数类型；`context` 保留缺省的 `:schema`，另以 `:inferred-schema` 展示编译器证据。查询不写回声明。要主动移除已确认冗余的声明，使用 `edit schema ... --clear` 后重新严格检查及运行测试；不要批量清除无法推断的边界。

## 源码函数别名复用同一调用契约

`def legacy-name module.api/new-name` 只是引用已有函数，不应绕过类型检查。
编译器沿已解析的源码引用找到真实函数，复用其参数数量、泛型关系、callback
期望类型与 optional/rest 检查；诊断保留调用方和别名表达式。别名链无需额外
wrapper，也不需要把类型放宽为 Dynamic。

这里只追踪编译后的直接引用，不执行任意 lazy 表达式来猜测函数，不把声明
的 Fn schema 当成任意实现已经满足契约的证明。循环引用不提供新证据。
这项调用检查修复不扩大 WASM 的一等函数支持范围。

## 局部类型断言不能覆盖已知矛盾

`assert-type` 不是值转换。已有局部类型与目标类型确定不兼容时，编译器在断言处报告
`E_ASSERT_TYPE_MISMATCH`，保留实际类型、期望类型与源码位置，而不是先改写类型信息，
等后续运算或后端执行才失败。例如 String `|hello` 不能通过断言变成 Number：

```bash
calcit eval "let ((x |hello)) (assert-type x 'Number)"
```

已证明的同型断言保持合法；断言为更宽的 Dynamic 也不应丢弃已有具体类型：

```cirru
let
    x 3
  assert-type x 'Number
  assert= 4 $ + x 1
```

此处拒绝的是已有证据中的确定矛盾，包括闭合容器 payload 类型不兼容。
开放值、未知类型与需要边界检查的关系不是同一结论；这项修复不表示所有 Dynamic 断言已完成深层验证。
外部数据应在输入边界使用真实 decoder，并处理失败；不要改成 `unsafe-coerce` 绕过已知错误。
通用断言、参数和返回值证明的后续收紧由 #1538/#1542 追踪。

## 递归参数遵守函数契约

局部 `loop` 从无 payload 的 Enum 变体开始时，编译器先根据词法尾部的 `recur` 输入推导未使用的泛型槽，再选择依赖该 payload 的静态方法。下面的值会推为 `Option<Number>`，native 与生成的 JS 都返回 `|7`：

```cirru
loop
    n 0
    value $ Option :none
  if (&< n 1)
    recur 1 $ Option :some 7
    (value .unwrap) .to-string
```

正反分支顺序、局部别名、闭包捕获及 `if` / `match` 合并保留同一类型关系，不需要把方法调用换成内部 proc。宏输入与展开结果仍按当前推导类型检查；每个逻辑宏调用只求值一次。显式 `Dynamic` payload 不会因此变成 Number，矛盾或递归嵌套的泛型方程仍拒绝。此推导不改变可变 Ref 的约束，也不扩展 backend 对局部递归形状的支持。

有固定参数的递归函数应声明 `Fn` schema。`recur` 的每个实参会按所在函数的参数位置和类型检查；即使参数个数相同，只要错序造成对应位置的类型不兼容，严格检查与 `analyze check-public` 也会报告到调用位置。相同或兼容类型的参数互换无法仅凭类型判定语义顺序，仍需测试验证。嵌套函数的 `recur` 遵守嵌套函数自己的参数契约。不要为了让错误的递归调用通过而把参数放宽为 `Dynamic`；应修正实参顺序或在确实需要开放输入的边界显式转换。

返回类型来自函数真正退出时的值。尾位置、词法函数归属、参数个数和实参类型都通过检查后，`recur` 只表示转移到下一轮，不要求它产生一个普通返回值。例如一个分支返回 `0`、另一个分支尾部 `recur` 的函数，可以由独立的数值退出证明返回 `Number`；`foldl-compare` 的 `true` / `false` 退出同理证明 `Bool`。`if`、`let`、`do` 的尾分支沿用这一推断，不需要为循环增加 `Dynamic` 或转换。

仅有 `recur`、没有独立退出证据的函数，不能用自身声明的返回类型给自己作证；嵌套函数也不能借外层函数的退出分支取得证明。开放或矛盾的退出值、未知 helper 的返回值，以及非尾部、错误参数的 `recur` 仍需修正。此推断不证明循环一定终止，也不表示所有 backend 都支持局部闭包递归；WASM 未支持的调用形状仍会明确拒绝。

普通 `defn` 在真正的尾位置直接调用自身时，编译器会在完成现有调用与返回类型检查后，复用 `recur` 的帧复用路径；例如 `list-match` 分支最后的 `(walk rest-items)` 可以遍历长列表而不增长 native 调用栈。此优化只适用于解析到当前顶层定义的直接调用，以及 `if`、`let`、`match` 的尾分支；被局部变量遮蔽的同名函数、间接调用、嵌套函数和非尾调用不改写。带可选或剩余参数等特殊参数形式的函数暂不自动改写。显式 `recur` 仍适合需要清楚表达循环意图的地方。

## 可变 Ref 的空初值

`Ref<T>` 的 payload 类型在构造时确定，之后的 `reset!` / `swap!` 只按该类型检查，不会反过来改写它。初值是无 payload 的变体（如 `Option :none`）时，需要在初始化表达式上给出 `Option<T>` 上下文：

```cirru
let
    cell $ ref $ assert-type (Option :none) $ :: 'Option 'Number
  reset! cell $ Option :some 7
  assert= 7 $ option:unwrap-or (deref cell) 0
  reset! cell $ Option :none
  assert= 0 $ option:unwrap-or (deref cell) 0
```

同一个 Ref 经局部别名写入 `Option :some |wrong` 时，同样报告 `W_RESET_ARG_TYPE_MISMATCH`。省略上下文时，`ref (Option :none)` 的 payload 槽固定为 Never，下面的写入会被拒绝，诊断会给出上面的初始化写法：

```cirru.no-check
let
    cell $ ref $ Option :none
  reset! cell $ Option :some 7
```

这样 Ref 的类型只由构造位置决定，与后续写入的出现顺序无关，也不会把尚未推导的槽当作 Dynamic。`loop` 参数可以从 `recur` 输入推导空变体的 payload，但 Ref 不按写入推导。

## JavaScript 可空值的容器合同

`JsNullish<T>` 表示 JavaScript 边界上可能缺席的值。已经证明为 `T` 的值和 `nil` 可以进入这个合同；同一关系递归适用于不可变 List、Map 与 Set，不需要逐层插入转换。例如一个保存 `Map<Tag,JsNullish<Number>>` 的 Struct 字段可以接收 `{} (:count 1)` 或 `{} (:count nil)`，具体回调同样可以存入可空回调表，其参数、返回值和 arity 仍按完整签名检查。

反方向仍需要检查：`JsNullish<T>` 不能直接作为 `T` 使用，Dynamic 元素也不能仅凭容器声明变成具体元素。可变 `Ref<T>` 保持不变性，不能把 `Ref<Number>` 扩大为 `Ref<JsNullish<Number>>` 后写入 `nil`。这项关系不把历史 `Optional<T>` 与 `JsNullish<T>` 混同，Calcit 自身的缺席值继续使用名义 `Option<T>`。

声明返回 `JsNullish<T>` 的函数按各个退出值检查。例如 `if present? value nil` 在 `value` 已证明为 `T` 时满足声明；反向分支、`match` 的已知 payload 和函数尾部 `let` 的局部别名采用同一检查。无需为了中间合并出的遗留 Optional 插入转换。初始化值按父作用域验证，新绑定和 match payload 遮蔽同名外层变量的证据。返回检查不修改局部变量的推断类型，也不把 raw primitive 返回的 `Optional<T>` 当成 `JsNullish<T>`。

已知字段、返回或显式 `assert-type` 合同也会逐项检查 List、Map、Set 字面量。例如 `Map<Tag,JsNullish<EventHandler>>` 可以接收 `{} (:click typed-handler) (:focus nil)`，嵌套集合采用同一规则；每个回调仍须具有兼容的完整签名。需要保存并复用混合集合时，可以在字面量处用 `assert-type` 明确检查合同。没有期望合同的异质集合仍按既有规则推导，开放的 Map 参数不会因此获得具体成员类型。检查不改变运行值，异步返回也不会自动等待集合中的 Promise。

## Dynamic 是边界，不是默认多态

普通泛型函数会按实参顺序传递已知的类型关系。例如 `filter-map-kv` 接收 `Map<String,Number>` 时，后续回调的参数可推为 `String` 和 `Number`；回调返回的 `MapEntryDecision<R,S>` 也保留泛型实参，让结果 `Map<R,S>` 继续约束后续写入。无需为了传递这些关系手工增加 `hint-fn` 或 `assert-type`。如果输入本身是显式 `Dynamic`，推断不会凭空把它收窄为某个具体类型，仍应在真实边界处 decode 或 narrow。

`Dynamic` 适合 JS FFI、框架开放数据、宏和确实无法提前知道的外部输入。普通函数不要用多个 `Dynamic` 表示“它们应该是同一个类型”：输入和返回关联时用 `:generics` 与 TypeVar；只需要能力时用 trait 与 `:where`；同质集合写出元素类型；有限异构数据定义为 Enum；可缺失值使用 `Option<T>`，带失败信息使用 `Result<T, E>`。

Dynamic 表示用户明确选择的开放 Calcit 值，不等于编译器尚未推断出的 Unknown/Unresolved，也不等于宿主拥有的
`JsObject`。开放值可以被保存、传递、放入开放容器或包装进 `Option<Dynamic>`；只有将其当成 Number、String、
具体 Struct/Enum 或某个 trait 能力使用时，才需要 decode、narrow 或显式 unsafe boundary。真正参数化且不观察
内容的泛型函数可以携带 Dynamic；声称具体内容关系但没有证据的调用继续拒绝。

`&trait-call Trait :method receiver ...` 用于消除不同 trait 的同名方法歧义，不是对 receiver 的强制转换。严格检查会拒绝无法静态识别的 trait/method、已知 Fn 方法签名的错误参数个数，以及已知内建 receiver 上确实不存在的 impl；例如 Number 不会因为显式写了 `Countable` 就获得 `.count`。局部 `impl-traits` 组合与具名泛型的附着来源目前尚不能在所有路径完整保留，缺失的静态证明不能误报成“确定没有 impl”；这类调用仍由运行时核对。普通业务优先使用带 schema 的接收者方法，让现有 trait 关系参与推断。

普通执行、编译和严格检查只报告可执行的 warning/error，不计算 Dynamic 比例。迁移存量代码时再显式查询具体位置：

```bash
calcit analyze check-types --summary-only
calcit analyze weak-types --only schema-dynamic,unresolved-type-slot,code-dynamic --intent unresolved --format edn
```

## 解析成功不等于数据类型验证成功

String 的 `.parse-cirru-edn` 返回 `Result<Dynamic,String>`：`:ok` 只说明文本符合 Cirru EDN 语法，
不证明内部元素符合业务类型。`.parse-json` 也是开放协议边界；JSON 不是 Calcit 的默认数据格式。
查询显示 `open` 时，不应凭调用名字、声明返回类型或浅层 `assert-type` 把 payload 当成已验证的具体值。

已知目标形状时，优先在输入边界使用 `try-parse-cirru-edn-as`，由现有 decoder 真正检查每层数据。
若数据已经作为开放值保存或传递，再使用 `try-decode-map-as`；它不只接受 Map，也能验证声明的 List 等闭合形状。
两个入口都返回 `Result<T,String>`，业务必须处理失败，而不是自动插入 unwrap、默认值或 unsafe cast。

```cirru
assert=
  Result :ok $ [] $ [] 1 2
  try-parse-cirru-edn-as "|[] ([] 1 2)" $ :: 'List $ :: 'List 'Number
match
  try-parse-cirru-edn-as "|[] ([] 1 |bad)" $ :: 'List $ :: 'List 'Number
  (:err message)
    assert= true $ message .includes? |[0][1]
  (:ok _) (raise "|无效的嵌套 Number 不应通过验证")
```

后一例语法合法，但第二层 List 的第二个元素是 String；错误保留 `[0][1]` 路径，便于 Agent 定位，
不依赖整段错误文本或给开放解析套 nominal wrapper。这些深层正反例在 native 和生成的 JS 上共享相同 Calcit 测试。
当前 WASM typed EDN parser 只支持已声明的标量及顶层同质标量集合子集；嵌套集合仍明确拒绝，
本例不表示新增 WASM 支持，也不替代 #1538 的通用断言/调用证明收紧。

## Schema 类型别名在各后端一致展开

非 Dynamic definition schema 可以通过完整的 `'namespace/definition` 名称作为类型别名使用。别名只复用底层类型关系，不创建新的运行时包装；需要名义身份时仍应使用 `defstruct` 或 `defenum`。

```bash
calcit edit def app.schema/Items --code 'quote $ def Items &unit'
calcit edit schema app.schema/Items --code "quote \$ :: 'List 'Number"
```

此后 `'app.schema/Items` 可用于 Struct 字段、Enum payload、callback 或 trait 边界。静态检查、Native 构造验证和 JavaScript codegen 都按同一底层 schema 判断值；不要为 Native 单独增加 coercion。类型别名不能接收泛型实参，循环别名也不会退化成 Dynamic，而是作为无法证明的类型关系拒绝。

## 数值 refinement 只携带边界证明

需要与外部 ABI 的明确宽度对齐时，可使用 `'Int8`、`'UInt8`、`'Int16`、`'UInt16`、`'Int32`、
`'UInt32`、`'Int64`、`'UInt64`、`'Float32` 和 `'Float64`。它们不是新的运行时数值族：native 仍使用
`f64`，JavaScript 仍使用 `Number`。refinement 可以安全地作为 `'Number` 使用；普通 `'Number` 不会隐式收窄，
普通算术结果也回到 `'Number`，避免在数值变化后继续保留失效的范围证明。

从普通数值进入明确边界时，使用 `number->int8`、`number->uint8`、`number->int16`、
`number->uint16`、`number->int32`、`number->uint32`、`number->int64`、`number->uint64`、
`number->float32` 或 `number->float64`。这些函数返回 `Result<目标类型,String>`，不会截断、环绕或静默舍入：
整数目标拒绝非整数、NaN、Infinity、符号错误和越界值，`Int64`/`UInt64` 只接受 JavaScript 与 `f64`
都能连续精确表示的安全整数范围；`Float32` 只接受能原样往返的值。

```cirru
let
    port-result $ number->uint16 8080
  result:ok? port-result
```

当前 source-level refinement 与受检转换在 native、JavaScript 后端一致。Component contract 与 Canonical ABI
lowering 也保留这些宽度：整数使用 `i32` / `i64`，浮点数使用 `f32` / `f64`；进入统一 `Number`
表示前后都会检查范围、整数性、符号与精确性，失败直接 trap，不会偷偷降级、截断或舍入。

兼容性的多态 collection facade 可能仍在 core schema 中保留局部 `Dynamic`，或者使用彼此独立、无法表达容器成员关系的泛型，但已知 receiver 会在预处理阶段专门化。例如 `update` 对 `List<T>` 要求 `Number` 索引和 `T -> T` updater，对 `Map<K,V>` 要求 `K` 键和 `V -> V` updater；Struct 则按静态字段类型检查。`filter`、`any?` 与 `every?` 对 List/Set 要求 `T -> Bool` predicate，`each` 则约束 callback 输入为 `T`、允许任意返回类型；`map` 对 List/Set 要求 `T -> U` mapper，并把 Set receiver lowering 到 `&set:map`。`foldl` 与 `reduce` 从初始值恢复 accumulator `U`，并要求 reducer 为 `U, T -> U`；原生 `foldl` 只有在 reducer 具有具体且兼容的 `Fn` 签名时才把初始 accumulator 类型保留为返回类型，`DynFn` 仍推断为 `Dynamic`。普通 `apply f args` 只会在 `args` 是非 Dynamic 的同质 `List<T>`、`T` 能满足 `f` 的全部 fixed/rest 输入、且展开长度能证明 callable arity 时恢复 `f` 的具体或泛型返回类型；若参数位置异构、固定参数调用的 list 长度未知、callable 未知，或存在 trait-bounded 泛型，则兼容返回仍为 `Dynamic`，应改为直接调用、先归一化参数，或在审核过的开放边界显式保留 Dynamic。双参数 `sort` 与 `&list:sort` 保留 `List<T>`，并要求 comparator 为 `T, T -> Number`；函数形式的 `&list:sort-by` 要求 selector 为 `T -> K`，同时保留 Tag 字段选择器兼容路径。List 的 `.apply` 要求函数列表中的每一项共享 `T -> U` 契约，并返回 `List<U>`；它的 direct/method 诊断会用 receiver/input 已绑定的 `T` 显示具体 callback 类型，异构输入或函数列表必须先归一化或拆成多次调用。`interleave` 同样只接受两份 `List<T>` 并返回 `List<T>`；异构数据必须先归一化，或在经过审核的开放边界显式声明 `List<Dynamic>`。单参数自然排序不受影响，Syntax collection 继续使用 phase-aware 开放契约。Map callback 接收运行时的异构 `[key value]` pair；迭代、predicate 与 fold 输入只承诺 `List<Dynamic>`，而 `map` 同时要求 callback 返回另一个 `List<Dynamic>` pair，不会把不同的 `K` / `V` 伪装成同一种成员类型。旧 `map-kv` 还允许 nil/任意 Enum 作为 drop sentinel，因此返回只能是显式兼容边界：兼容模式警告，strict 模式拒绝。typed code 应统一改用 `filter-map-kv`，以 `MapEntryDecision :keep key value` 或 `:drop` 让输出 key/value 与 callback payload 保持可证明关联。`get` 同样要求 List/String/Enum 的 `Number` 索引或 Map 的 `K` 键，`includes?` 要求 List/Set 的成员 `T`、Map 的值 `V` 或 String substring；`contains?` 要求 List/String/Enum 的 `Number` 索引、Map 的键 `K` 或 Set 的成员 `T`。`assoc` 会同时约束 List 的索引/成员、Map 的键/值、静态 Struct 字段的值类型，以及 Enum 的 `Number` payload index；Enum payload 可以异构，因此新值在没有精确 variant/slot evidence 时仍保持开放。`dissoc` 会检查全部 rest 参数：List 只能接收 `Number` 索引，Map 的每个键都必须是 `K`。用户函数 schema 的 `:rest` 会逐项检查。原生 proc 按运行时契约区分两类 typed variadic：`&map:dissoc`、`&list:concat` 与 `&merge` 会检查每一个 rest 参数，并在容器不匹配时显示完整成员类型；`[]` 与 `#{}` 的 `Variadic<T>` 只用于推断公共成员类型，异构字面量仍有意回退为 `Dynamic`。未标注的 inline callback 若能从函数体恢复返回类型，也会参与这项检查。不要把 receiver 擦除为 `Dynamic` 来绕过这些关系：在 FFI/open-data adapter 中先校验或转换，再进入集合操作。

对已知 receiver 的集合调用，成员类型会先进入 inline callback 的参数与函数体，再由同一份 contract 检查调用参数和推断返回值。`any?`、`every?` 与 `each` 因而不需要让 callback 暂存未实例化的泛型，也不需要为使用它们的宏增加例外。

## 严格检查与迁移期 quality baseline

新项目以默认严格预处理和实际目标测试作为发布门禁：

```bash
calcit calcit.cirru --check-only
calcit calcit.cirru --entry test
```

`analyze check-types` 与 `analyze weak-types` 只帮助定位迁移清单，不决定程序是否类型正确，也不输出 Dynamic 比例、shape/family 排名或另一套关系判断。需要清理时使用 kind、intent、definition、path 和 detail 回到源码；值能否进入 typed code 只由严格预处理诊断决定。

对于省略 schema、且编译器已经能够证明完整契约的封闭 helper，`check-types`、`weak-types` 和 `quality` 使用同一份推断结果；增量分析也遵循这一规则。删除这类冗余标注不应增加迁移债务。分析不会把推断结果写回源码，也不会替换显式 `Dynamic`；推断失败的定义仍按原有缺失契约报告。

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

多个连续的 Option 或 Result 步骤嵌套使用接收者 `.and-then`，让错误类型的转换
保持可见：

```cirru.no-check
let
    source $ fs:path |data.cirru
    content-result source.read-text
  content-result.and-then $ fn (content)
    (parse-data content) .and-then $ fn (data)
      save-data data
```

先用 `fs:path` 把 UTF-8 String 提升为 nominal `FsPath`，再调用 `.read-text`、
`.read-dir`、`.walk-dir` 或 `.write-text!`，这些方法返回 `Result<...,String>`。
String 本身不携带文件系统语义；旧 `try-read-file` / `try-write-file` 已退役，
先通过 `fs:path` 构造路径再调用 `.read-text` / `.write-text!`。旧 `.write-text` 暂留兼容；`try-read-dir`
与底层 raising procedures 暂留为兼容入口。
这些文件效果支持 native 与生成的 JavaScript。WASI 0.3 Component command 目前支持基于
preopen 的 `.read-text` / `.write-text!`（UTF-8，最多 4 MiB）。写入采用 create + truncate，
失败可能留下截断或部分内容，不是原子替换；超限在打开前拒绝。`.read-dir` 和 `.walk-dir`
仍待实现。core WASM 明确拒绝宿主文件效果。

需要同步等待时使用 `wait-ms`，让缺少宿主能力或等待失败继续保留在
`Result<Unit,String>` 中。参数是明确的整数毫秒，不接受隐式舍入；异步调度仍使用
独立的 callback/task API，不把两种控制流混成一个动态接口。

Native 异步 FFI capability 也遵循相同边界原则。模块适配层用 `ffi:task`、
`ffi:response` 把不透明 AnyRef 提升为 nominal `FfiTask`、`FfiResponse`，业务层调用
`.cancel!`、`.cancel-with!`、`.resolve!`、`.reject!`。旧名暂留兼容。raw 字段保持 `Dynamic`，但
reason/payload 使用方法级泛型，因此不会为了宿主编码而抹掉调用侧类型。底层
`&ffi-task-cancel`、`&ffi-response-*` 只作为适配与兼容入口。

普通组合函数以接收者方法
作为公开形式，`option:*` / `result:*` 直接函数调用主要保留给 core lowering。

## get-in / assoc-in / update-in

`get-in` 是可能失败的开放数据访问，返回 `Option<T>`。当接收者是完整类型的嵌套 Map、路径是非空字面量时，编译器会把 `get-in`、`assoc-in` 和 `update-in` 展开为类型化的直接 `get` / `assoc` 链，不再经过运行时动态路径遍历；调用参数仍按源码顺序各求值一次。`update-in` 的 updater 接收 `Option<T>`，因此缺失叶子不会退回 nil。

动态接收者、动态路径和混合容器仍是明确的兼容边界，会保留动态路径 API。新代码在数据形状已知时优先使用直接 `.get`、Option 组合和名义字段访问；只有开放数据才使用路径 API。不要用路径函数绕过 Struct 字段检查：Struct 应使用 `(:field value)` 或 `value.:field`，字段可缺失就把字段声明为 `Option<T>`。

`Map<K,V>` receiver 的 `.get`，`List<T>` receiver 的 `.get`、`.first`、`.last`，以及 String、Enum receiver 的 `.get`、`.nth`、`.first`、`.last`，会在 preprocess 阶段降低为对应类型的 `&scope:*` primitive 与显式 Option 构造，不再执行通用函数中的 receiver type predicates。对应的前缀形式保持兼容并使用同一 lowering；业务代码优先使用 receiver 形式来保留类型意图。

对 `update-in` 的缺失值给默认值或明确处理 `Option :none`，不要无条件 unwrap：

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
的字段可以省略，Calcit 会补成 `Option :none`。需要显式控制所有字段时使用
`%{} Profile ...` 并完整提供字段。`%{}?` 与底层 `&%{}?` 已退役，
包括 `--compat-types` 在内都会以 `E_PARTIAL_STRUCT_NIL_FILL` 拒绝；不要用隐式 `nil`
模拟可缺失字段。

已知 Enum 定义时使用头部调用：

```cirru.no-check
Option :some value

Result :err message
```

Calcit 会根据 Enum 定义检查 variant 和 payload，并在预处理阶段生成命名构造。`%::` 保留给显式 prototype、动态跨模块构造和兼容旧代码的边界。
