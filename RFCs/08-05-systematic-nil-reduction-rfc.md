# 系统性减少 nil：类型驱动迁移 RFC

状态：Partial（Phase 1/2 已开始）
日期：2026-08-05

## 目标

在不掩盖现有运行时行为的前提下，让类型系统逐步阻止业务代码依赖 `nil`：

- 无业务返回值使用 `Unit`（运行时暂仍由 `nil` 承载）；
- 正常的“可能缺失”最终使用名义类型 `Option<T>`；
- 可恢复失败使用 `Result<T, E>`；
- JavaScript `null`/`undefined` 只通过 `JsNullish<T>` 进入类型系统；
- `Optional<T>` 不再是公开 API 类型，只作为编译器识别旧 schema 与内部自举债务的兼容表示；
- 参数可以省略是调用约定，不再借用 `Optional<T>` 表示。

这不是立即删除运行时 `nil`。迁移顺序是先让类型契约诚实，再由诊断推动调用方显式处理，最后收紧遗留运行时行为。

## 已确认的现状

当前实现已经具备迁移所需的主要构件：

- `CalcitTypeAnnotation::Optional(T)` 能表达 `T | nil`；
- `CalcitTypeAnnotation::JsNullish(T)` 独立表达 JavaScript 宿主空值，不与 Optional/Option 相互匹配；
- `nil` 推断为 `Unit`，且 `Unit` 可以匹配 `Optional<T>`；
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

`Unit` 只用于副作用操作或明确没有有意义返回值的表达式，例如写文件、注册 watcher。运行时目前仍由 `nil` 承载 Unit，但源码不应因此显式返回 `nil` 或 `;nil`：空函数体直接声明 `Unit`，有副作用的函数让最后一个 Unit effect 自然成为返回项。只有语法必须提供占位节点时才保留 `;nil`，业务代码不得把它当缺失值容器。

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
- 公开 core 的 `first`、`nth`、`get` schema 继续暴露 Optional；内部 `&list:first`、`&list:nth`、`&map:get` 的 Dynamic proc 契约及低层专用推断暂保留兼容行为；
- `analyze weak-types` 将裸 `nil` 与 `;nil` 都纳入审计，仅在结构上可证明的返回位置读取函数契约，并区分 `declared-unit`、`declared-optional` 与 `unresolved`；JSON 对后两类迁移债务发出 `W_NIL_TYPE_DEBT`；
- `analyze.weak-types` 的机器协议升级到 schema v2，避免旧消费者在 v1 下错误接受新增的封闭 intent/diagnostic 枚举；
- 修正既有 `optionally` 桥接函数的契约为 `Optional<T> -> Option<T>`，为遗留 nullable 边界提供不丢失类型关系的显式出口；
- 为上述规则增加单元测试。

Phase 1 的类型契约修正本身不改变相关过程的运行时返回值；`rest` / `butlast` 的空集合修正与后续名义 API 切换则是单独记录的 breaking change。

低层专用推断不能通过批量 `unsafe-coerce` 清理：core 宏在检查列表形状后读取 AST，当前类型系统尚不能携带“非空列表”及 guard clause 终止证据，强制转换会把泛型元素退化成 Dynamic。该兼容点必须在引入非空集合/控制流证据后移除；公开 `first`、`nth`、`get` 的 schema 仍保持 Optional，不把这一内部例外扩散成新 API。

nil 审计也坚持证据边界：返回的 `do` 只有最后一项继承返回契约，返回的 `if` 只有结果分支继承契约；中间步骤、集合内容和尚未建模的控制流仍标成 `unresolved`。`declared-unit` 不计入迁移债务，`declared-optional` 则继续提示迁移到 Option/Result。

### Phase 2：建立名义安全 API

- 直接将原有 `find`、`find-index`、`index-of` 改为返回 `Option`，让旧调用在结果消费处产生明确的类型迁移提示；
- 将公开 `parse-float` 改为 `String -> Result<Number,String>`，`:err` 保留原始非法输入；nullable 底层过程改名为内部 `&parse-float`；
- 将公开 `get-env` 改为 `String -> Option<String>`，删除旧的第二个默认值参数；迁移时使用 `option:unwrap-or`，nullable 底层过程改名为内部 `&get-env`；
- `optionally` 仅保留为 core/internal 遗留 Optional 到 Option 的桥接，不接受 JsNullish；
- 泛型实参无法确定时，只把未绑定 payload 降为 Dynamic，保留 `Option<Dynamic>` / `Result<...,Dynamic>` 等外层名义类型，避免整个结果退化成 Dynamic；
- 对 `some?`/`nil?`、位置式 `get`/`nth` 以及底层 `&compare` 消费名义 enum 的旧 nullable 用法报告 `W_NOMINAL_ENUM_LEGACY_USE`，提示改用 Option 方法、unwrap 或 `tag-match`；
- core schema 成为公开函数契约，Rust proc 签名负责底层过程契约，并增加一致性审计。

`first`、`last`、`nth`、`get` 的 core 自举实现暂时仍由编译器内部识别 Optional 债务：它们参与宏展开，直接切换会让 `let` 等宏在加载阶段收到 Option。后续需先把宏实现迁到明确的低层原语，再切换公开契约。该例外不允许出现在非 core 的公开函数 schema。JS FFI 已独立为 `JsNullish<JsObject>`，不会跟随这些 API 自动变为 Option，也不能通过 `optionally` 擦除边界。

### Phase 3：typed code 严格化

在启用类型检查的代码中逐项收紧：

- 禁止将 `Optional<T>` 直接传给要求 `T` 的参数；
- 值上下文中的 `if` 必须有 else，或显式返回 Option；
- 条件表达式要求 Bool，不再依赖 nil truthiness；
- `first`、`rest`、`get`、`map`、`filter`、`to-list`、`to-map` 等不再接受 nil 作为正常集合；
- 部分 Record 省略字段必须由字段类型或显式默认值许可。

每条规则先提供稳定诊断码和修复提示，再升级为错误。无类型代码继续走兼容路径，直到单独决定移除窗口。

### Phase 4：收缩运行时 nil

- 将公共安全 API 的返回值切换为 Option/Result；
- 删除已无调用方的 nil-tolerant 分支；
- 删除 core 自举完成后剩余的 Optional 公开解析能力，仅在迁移工具中识别旧 schema；
- 审计 JS/WASM 后端，确保名义类型的表示和分支行为一致。

## 类型提示修复策略

迁移应优先给出局部、可机械执行的建议：

- `Optional<T> -> T`：提示先用 `some?`/`nil?` 收窄，或转换为 Option；
- `JsNullish<T> -> T`：提示使用 `js-present?`/`js-nullish?` 收窄，验证 opaque payload，并只在显式需要时调用 `js-nullish->option`；
- 缺少 else：提示补 else、改为 `Option<T>`，或明确声明 `Optional<T>` 兼容边界；
- nil 作为集合：提示在来源处处理缺失，不自动替换成空集合，因为二者业务语义不同；
- 可省略参数处显式传 nil：提示省略参数，或修改参数值类型为 Optional；
- 解析返回 Optional：提示改用后续的 Result API，以保留错误信息。
- 名义 Option 仍用 `some?`/`nil?` 或 tuple 位置读取：提示改用 `option:some?`、`option:none?`、`option:unwrap-or` 或 `tag-match`。

自动修复不得把 nil 无条件替换为空集合、0、空字符串或 false。

## 兼容性与验收

每个阶段必须满足：

1. proc 运行时行为与声明返回类型一致；
2. 参数 arity 与参数值类型分别测试；
3. Native、JS、WASM 的可观察结果一致；
4. 新诊断包含稳定代码、位置和明确修复方向；
5. examples、core、真实外部项目分别统计新增诊断，不能只依赖单元测试；
6. 任何运行时破坏性变更都必须有迁移期和独立发布说明。

## 非目标

- 不把所有 `nil` 机械替换成 `%none`；
- 不在同一阶段修改全部 core 集合语义；
- 不用 Dynamic 掩盖无法建模的缺失；
- 不承诺 Option/Result 与 FFI 的零成本表示，边界转换需要单独设计和测试。
