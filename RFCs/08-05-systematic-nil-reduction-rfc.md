# 系统性减少 nil：类型驱动迁移 RFC

状态：Complete（下一版本发布候选；公开 nil 契约已冻结）
日期：2026-08-05

## 目标

在不掩盖现有运行时行为的前提下，让类型系统逐步阻止业务代码依赖 `nil`：

- 无业务返回值使用 `Unit`，新代码以独立的 `&unit` 值表示；
- 正常的“可能缺失”最终使用名义类型 `Option<T>`；
- 可恢复失败使用 `Result<T, E>`；
- JavaScript `null`/`undefined` 只通过 `JsNullish<T>` 进入类型系统；
- `Optional<T>` 不再是公开 API 类型，只作为编译器识别旧 schema 与内部自举债务的兼容表示；
- 参数可以省略是调用约定，不再借用 `Optional<T>` 表示。

这不是立即删除运行时 `nil`。迁移顺序是先让类型契约诚实，再由诊断推动调用方显式处理，最后收紧遗留运行时行为。

## 下一版本稳定性冻结

下一版本是 nil 迁移的最终 breaking window。该版本合并、升级并发布后，以下规则进入兼容性承诺：

- 公开缺失统一返回 `Option<T>`，公开可恢复失败统一返回 `Result<T,E>`，副作用返回 `Unit`；
- 公开 schema 不得出现 `Optional<T>`。`--strict-types` 会发出 `E_LEGACY_OPTIONAL_SCHEMA`，普通模式保留迁移警告；该规则同样约束 `calcit.core` 的公开定义；
- 名称以 `&` 开头的 raw primitive 属于 semver-private 实现细节，允许在内部使用 nullable 表示，但不得直接挂入公开方法表；
- 已生成 JS bundle 引用的 npm runtime export 属于 codegen ABI：typed wrapper/raw proc 改名时必须保留兼容 export，并由 runtime identity test 覆盖；类型系统负责阻断重新编译的旧 FFI 源码，但不能替代旧 bundle 的装载兼容性；
- 新增查询 API 不得先返回 nil、再在后续版本改成 Option。类型和所有后端实现必须在首次发布时一致；
- `analyze weak-types --only code-nil --intent unresolved,declared-unit,declared-optional` 必须对 core 返回零结果；
- 以后若要改变这些名义返回类型，只能作为独立的非 nil 设计变更处理，不能再以“清理遗留 nil”为由制造连续 breaking change。

本次最终迁移表：

| 旧契约 | 下一版本契约 |
| --- | --- |
| `first` / `last` / `nth` / `get` / `get-in` 返回值或 nil | `Option<T>` |
| Map/Set `.destruct` 暴露 nullable tuple | `MapDestruct<K,V>` / `SetDestruct<T>` |
| Record `.nth` 暴露跨后端不稳定字段顺序 | 移除公开方法；字段名 `get -> Option<T>` |
| `parse-float` 返回 number 或 nil | `Result<Number,String>` |
| `get-env` 返回 string 或 nil | `Option<String>` |
| `when-let` 返回 body 或 nil | `Option<R>` |
| `update-in` updater 接收值或 nil | updater 接收 `Option<T>` |
| 非穷尽 `case` / `cond` 隐式返回 nil | 明确报错；`cond` 要求最终 `true` 分支 |
| `dissoc-in` 空路径返回 nil | 空路径保持输入值不变 |

## 已确认的现状

当前实现已经具备迁移所需的主要构件：

- `CalcitTypeAnnotation::Optional(T)` 能表达 `T | nil`；
- `CalcitTypeAnnotation::JsNullish(T)` 独立表达 JavaScript 宿主空值，不与 Optional/Option 相互匹配；
- `nil` 与 `Unit` 过去共享静态表示，导致声明为 `Unit` 的副作用函数仍可能实际返回 nil；
- `nil?` / `some?` 已支持分支内的 Optional 类型收窄；
- core 已定义 `Option<T>`、`Result<T, E>` 及基本组合函数；
- `get` 对 List、Map 和 String 已采用可能缺失的返回推断。

主要障碍不是缺少类型，而是边界契约与历史兼容行为不一致：

1. 内建过程曾用参数类型上的 `Optional<T>` 同时表示“参数可省略”和“传入值可为 nil”；
2. 部分运行时会返回 nil 的过程仍声明为裸类型或 Dynamic；
3. `first`、`rest`、`get`、集合转换及无 else 的 `if` 等旧行为，使 nil 可以在未声明的情况下扩散；
4. core schema、Rust proc 签名、专用类型推断之间存在重复事实来源。

## 语义边界

### Unit

`Unit` 只用于副作用操作或明确没有有意义返回值的表达式，例如写文件、注册 watcher。新代码显式返回 `&unit`；它与 `nil` 不相等、不会通过 `nil?`，JavaScript 后端生成 `void 0`（而 `nil` 仍生成 `null`）。`Unit` 标注不再接受 nil；`nil` 具有独立的 `Nil` 静态类型，并且只有 Nil 可以进入遗留 `Optional<T>`。`;nil` 保留为返回 Nil 的兼容宏，不能用作 Unit 标记；副作用函数应以 `&unit` 或最终得到 Unit 的 `do` 收尾。

### Optional<T>（遗留内部表示）

`Optional<T>` 只用于识别旧 schema 和尚未完成自举迁移的 core 内部契约。非 core 的公开函数 schema 不再允许声明 Optional；普通缺失必须使用 Option，失败使用 Result，无业务返回使用 Unit。

### JsNullish<T>

JS FFI 是无法立即消除的宿主空值边界。原生属性读取、方法调用、`aget`/`js-get` 与未声明的 `js/...` 调用返回 `JsNullish<JsObject>`：JsNullish 表达 `null`/`undefined`，不透明的 `JsObject` payload 仍要求调用方验证、转换或在可信契约处 `unsafe-coerce`。使用 `js-nullish?`/`js-present?` 收窄；旧 `nil?`/`some?` 会产生专用迁移诊断。

`JsNullish<T>` 不匹配 `Optional<T>`，因此不能传给通用 `optionally` 静默包装。只有显式 `js-nullish->option` 可以建立 `Option<T>`，且该转换不负责验证 opaque payload。

### Option<T>

新设计的查询、解析和集合查找 API 应返回 `Option<T>`。`%some` / `%none` 是普通名义值，不依赖 truthiness，也不会与 Unit 混淆。

### Result<T, E>

格式错误、IO 错误、解码失败等带原因的失败应使用 `Result<T, E>`。只有真正无错误信息价值的缺失才使用 Option。

### 可省略参数

参数可省略属于函数/过程的 arity 元数据。参数声明为 `T` 时，若调用方提供该位置，值必须匹配 `T`；只有参数值类型明确为 `Optional<T>` 时才允许显式传 nil。

## 分阶段实施

### Phase 1：契约诚实化与语义拆分

- 内建过程用独立 arity 元数据表示末尾可省略参数；
- proc 参数检查不再剥离 `Optional<T>`；
- 将底层 `&parse-float` 和 `&get-env` proc 的 nullable 返回标成 Optional，作为公开名义 API 之下的兼容边界；
- 将 `rest` / `butlast` 对空 List 的结果统一为同类型空 List，并同步 Native、JS、WASM；`rest` 与 `empty` 的公开契约改为 `T -> T`，因此确定存在的集合/String 不再被无条件提升为 Optional，而显式 Optional/nil 输入仍保留其可空类型；
- 公开 core 的 `first`、`last`、`nth`、`get`、`get-in` schema 返回名义 `Option`；内部 `&list:first`、`&list:nth`、`&map:get` 等 raw primitive 保留 nullable 表示并标记为 internal；
- `analyze weak-types` 将裸 `nil` 与 `;nil` 都纳入审计，仅在结构上可证明的返回位置读取函数契约，并区分 `declared-unit`、`declared-optional` 与 `unresolved`；三类都属于迁移债务并由 JSON 发出 `W_NIL_TYPE_DEBT`；
- `analyze.weak-types` 的机器协议升级到 schema v2，避免旧消费者在 v1 下错误接受新增的封闭 intent/diagnostic 枚举；
- 修正既有 `optionally` 桥接函数的契约为 `Optional<T> -> Option<T>`，为遗留 nullable 边界提供不丢失类型关系的显式出口；
- 为上述规则增加单元测试。

Phase 1 的类型契约修正本身不改变相关过程的运行时返回值；`rest` / `butlast` 的空集合修正与后续名义 API 切换则是单独记录的 breaking change。

低层专用推断不能通过批量 `unsafe-coerce` 清理：core 宏在检查列表形状后读取 AST，当前类型系统尚不能携带“非空列表”及 guard clause 终止证据。此类宏改用明确的 `&` raw primitive；公开包装器始终保留 Option 外层，不把内部 nullable 表示扩散成 API。

nil 审计也坚持证据边界：返回的 `do` 只有最后一项继承返回契约，返回的 `if` 只有结果分支继承契约；中间步骤、集合内容和尚未建模的控制流仍标成 `unresolved`。`declared-unit` 提示改为真正的 `&unit`，`declared-optional` 则继续提示迁移到 Option/Result。

### Phase 2：建立名义安全 API

- 直接将原有 `find`、`find-index`、`index-of` 改为返回 `Option`，让旧调用在结果消费处产生明确的类型迁移提示；
- 将公开 `parse-float` 改为 `String -> Result<Number,String>`，`:err` 保留原始非法输入；nullable 底层过程改名为内部 `&parse-float`；
- 将公开 `get-env` 改为 `String -> Option<String>`，删除旧的第二个默认值参数；迁移时优先使用接收者方法 `.unwrap-or`，nullable 底层过程改名为内部 `&get-env`；
- `optionally` 仅保留为 core/internal 遗留 Optional 到 Option 的桥接，不接受 JsNullish；
- 反射 API `tuple-enum`、`impl-origin` 返回名义 `Option`，nullable 的 `&tuple:enum`、`&impl:origin` 只保留为内部原语；`record-struct` 则收紧为必然返回 `Struct`；
- `destruct-list/map/set/str` 从匿名 `:: :some/:none` tuple 升级到参数化的名义 `*Destruct` enum，让 variant 载荷参与类型检查；
- 泛型实参无法确定时，只把未绑定 payload 降为 Dynamic，保留 `Option<Dynamic>` / `Result<...,Dynamic>` 等外层名义类型，避免整个结果退化成 Dynamic；
- 对 `some?`/`nil?`、位置式 `get`/`nth` 以及底层 `&compare` 消费名义 enum 的旧 nullable 用法报告 `W_NOMINAL_ENUM_LEGACY_USE`，提示改用 Option 方法、unwrap 或 `tag-match`；
- core schema 成为公开函数契约，Rust proc 签名负责底层过程契约，并增加一致性审计。

core 自举宏已经迁到明确的 `&` raw primitive，公开 `first`、`last`、`nth`、`get` 不再承担 Optional 债务。JS FFI 独立为 `JsNullish<JsObject>`，不会跟随这些 API 自动变为 Option，也不能通过 `optionally` 擦除边界。

### Phase 3：typed code 严格化（已完成本 RFC 范围）

在启用类型检查的代码中逐项收紧：

- 禁止将 `Optional<T>` 直接传给要求 `T` 的参数；
- 非穷尽 `case` 运行时报错，`cond` 必须提供最终 `true` 分支；可能缺失的业务值显式返回 Option；
- 条件表达式要求 Bool，不再依赖 nil truthiness；
- `first`、`rest`、`get`、`map`、`filter`、`to-list`、`to-map` 等不再接受 nil 作为正常集合；
- 部分 Record 省略字段必须由字段类型或显式默认值许可。

每条规则先提供稳定诊断码和修复提示，再升级为错误。无类型代码继续走兼容路径，直到单独决定移除窗口。

### Phase 4：收缩运行时 nil（公开边界已完成）

- 公共安全 API 的返回值已切换为 Option/Result/Unit；
- 删除已无调用方的 nil-tolerant 分支；
- Optional 仅由迁移工具与 internal raw primitive 识别；公开 schema 由稳定诊断阻断；
- 审计 JS/WASM 后端，确保名义类型的表示和分支行为一致。

### Phase 5：Nil / Unit 运行时与 ABI 分离

- 为 `nil` 增加独立的 `Nil` 类型标注，禁止其匹配 `Unit`，同时保留 Nil 到遗留 `Optional<T>` 的关系；
- Native effect proc 返回真实 Unit；有意义的 mutation（例如 `reset!`）保留并声明其写入值，而不是伪造 Unit；
- JavaScript 后端用 `null` 表示 Nil、用 `undefined` 表示 Unit，并在相等、哈希、格式化、类型反射和 typed decode 中保持二者可区分；
- data-shape ABI 增加独立 Nil 节点并升级版本，防止旧 runtime 静默把 Nil 当 Unit；Cirru EDN 和 JSON 均拒绝序列化 Unit；
- `;nil` 的零参数 hint form 必须生成真正的 Nil 返回；`assert-type` / `assert-traits` 在 JS 中保留被断言的值；
- 对 js-ffi、Respo 与 Recollect 做组合验证，只迁移明确的 effect tail；DOM 缺失、宿主 nullish 和框架动态边界不机械替换。

实施跟踪见 [#428](https://github.com/calcit-lang/calcit/issues/428) 与 [#429](https://github.com/calcit-lang/calcit/issues/429)。

## 类型提示修复策略

迁移应优先给出局部、可机械执行的建议：

- `Optional<T> -> T`：提示先用 `some?`/`nil?` 收窄，或转换为 Option；
- `JsNullish<T> -> T`：提示使用 `js-present?`/`js-nullish?` 收窄，验证 opaque payload，并只在显式需要时调用 `js-nullish->option`；
- 缺少 else：提示补 else、改为 `Option<T>`，或明确声明 `Optional<T>` 兼容边界；
- nil 作为集合：提示在来源处处理缺失，不自动替换成空集合，因为二者业务语义不同；
- 可省略参数处显式传 nil：提示省略参数，或修改参数值类型为 Optional；
- 解析返回 Optional：提示改用后续的 Result API，以保留错误信息。
- 名义 Option 仍用 `some?`/`nil?` 或 tuple 位置读取：提示改用 `.some?`、`.none?`、`.unwrap-or` 或 `tag-match`。

自动修复不得把 nil 无条件替换为空集合、0、空字符串或 false。

## 兼容性与验收

每个阶段必须满足：

1. proc 运行时行为与声明返回类型一致；
2. 参数 arity 与参数值类型分别测试；
3. Native、JS、WASM 的可观察结果一致；
4. 新诊断包含稳定代码、位置和明确修复方向；
5. examples、core、真实外部项目分别统计新增诊断，不能只依赖单元测试；
6. 本 RFC 的破坏性变更集中进入下一版本发布说明；该版本以后禁止继续追加 nil 驱动的 breaking change。

## 非目标

- 不把所有 `nil` 机械替换成 `%none`；
- 不在同一阶段修改全部 core 集合语义；
- 不用 Dynamic 掩盖无法建模的缺失；
- 不承诺 Option/Result 与 FFI 的零成本表示，边界转换需要单独设计和测试。
