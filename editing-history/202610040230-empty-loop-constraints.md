# 空 Enum 循环参数约束

## 实际问题

空 Option 绑定使用内部 Never 保留无 payload 的证据后，`loop` 的初值把参数固定为 `Option<Never>`；后续 `recur` 写入 `Option<Number>` 被拒绝。真实 Respo 的重复 key 检测因此从原有 62/74 降到 61/74，不能靠业务补注解、删除测试或扩大 Dynamic 收尾。

## 决策

对于没有 source schema 或显式函数 hint 的函数，只给参数中已证明为空的 Enum 泛型槽补约束。遍历已预处理的词法尾部 `recur`，复用现有分支返回类型合并；词法 let、match 分支绑定使用已有 scope 推导。引入真实 payload 后，重检已经展开的 core 函数体，使参数节点、返回证明和各类型门禁使用同一个合同。

不能为了重检再次展开原 source：有日志等副作用的宏会重复执行。保留展开结果与引用身份，重新绑定局部节点，去除生成函数末尾的一份编译器签名，但保留 source 显式 hint。已索引的 match 使用现有分支读取器恢复等价分支对，再重新推导 payload；decoder 从保留的类型表达式重新派生内部 handle；引用与语法引用不重写。不为此启用宏缓存、跳过类型门禁或增加新的公开配置。

局部 `assert-type` 原来会直接消除为带类型的 Local，但依赖空 Enum 证明的断言不能提前消失：后续循环引入真实 payload 后，仍必须满足声明。复用空槽谓词，为这种断言暂时保留现有 AssertType core 形式及其原始空槽输入；原始输入只标识需要重检的断言义务，不再作为实际输入证明。重检时从新的词法 scope 刷新整个输入表达式，要求独立 proof，再执行原有 Local lowering；相矛盾的 payload 和未知 payload 分别沿用 `E_ASSERT_TYPE_MISMATCH`、`E_ASSERT_TYPE_UNPROVEN`。直接变量、条件表达式和捕获循环参数的闭包内都适用。不新建断言记录表，也不对整段循环启用额外审计模式。正常的具体断言保留既有 lowering。

只补仍为 Never 的直接 Enum 参数槽；具体参数、显式 Dynamic、公开函数合同和 Ref 均保持固定。不建立按 loop 或业务函数名授权的例外，不扫描原构造器形状补证据，不新增局部 provenance registry。不同 nominal 身份与不相容 payload 没有合法合并。多个参数间的传递会继续补剩余空槽，每次推进都消除至少一个直接空槽，不将递归地构造无限嵌套类型作为推导目标。若某次 transfer 缺少返回证据，不能沿用初始 Never：使用已有开放类型表示继续重检，具体字段必须要求真实证明；不能把缺失证据当作空 payload。

尾部循环转移不产生返回值，复用现有独立函数退出证明推导生成函数的返回合同。只有真正的退出分支能提供返回类型；纯 recur 循环不能自行获得具体返回证明。引入新约束后仍重新检查整个展开后的函数体，不将原来对无值的证明用于后来实际承载的值。

严格 `recur` 检查与独立退出证明使用相同的定向 proof，要求保持当前词法实例化，不按新调用为 rigid 泛型重新绑定；未知输入不能跳过校验。沿用 `W_RECUR_ARG_TYPE_MISMATCH`、expected/actual 与原 source 定位；非严格迁移模式保持原有兼容检查。

## 验证

在现有断言 runner 中通过结构化 CLI 添加 definition-attached Calcit 测试，native 与真实生成 JS 回放空 Option、任意用户 Enum、let 中的别名、match payload、decoder、普通 `.unwrap-or` 方法、合法显式断言与跨参数的两阶段传递。另用带日志的宏验证一次编译中只展开一次。十四个负例与既有递归字段负例复用七种 native/check/JS/WASM/WASI 入口，要求错误 payload 的具体使用、开放输出、混合 payload、错误 nominal、invariant Ref、显式合同，以及直接或闭包内相矛盾、缺少独立输入证明的断言被拒绝；开放 payload 的直接和条件表达式断言都覆盖。不产生应用产物，不修改 Snapshot。

decoder 的正例直接放在 native/JS 附带测试中，而不是留作独立程序定义。WASM 会收集独立函数，尚不支持的 decoder 不能污染后续无关的 nominal-write WASM fixture；保持后端的 unsupported 边界，不增加强制编译放行或类型豁免。

Ref 负例使用显式固定的 `Ref<Option<Dynamic>>` 合同，拒绝转入具体 `Ref<Option<Number>>`：前者允许写入任意 payload，不能因此放宽后者可变内容的类型。对应正例允许转入同样显式的 Ref 合同并仅存储开放数据。不把尚待完善的未注解空 Ref 推导写成永久拒绝契约，也不要求业务为此次循环推导修复补 Ref 注解。

真实 Respo 独立副本保持全部 74 项测试，循环回归恢复到 62/74，其余 12 项失败不由此改动掩盖。可变 Ref 的隐式写入推导仍是独立待解决事项，本补丁不宣布它或 milestone 已完成。完整 Cargo、Clippy 与集成结果以 PR 的最终 HEAD 记录为准。

## 后续边界

签名保留回归直接存入 core `hint-fn` 的附带测试并标记为 unit。默认工作流继续严格检查；兼容回放复用允许显式测试该边界的 `check-strict-default.mjs`，不为普通 runner 增加兼容开关或放宽入口扫描。结构化 CLI 写入同时规范化了两处既有括号/缩进，AST 语义不变。

Review 补充了两条同一重检路径的边界。尾部遍历遇到空 core 列表时只跳过该分支，继续收集其他分支的转移约束；native 附带测试覆盖两种分支顺序，空分支不被执行。裸空 core 表达式目前不能生成有效 JS，独立 tag 只回放 native，不混入 JS 正例，也不借此次修复新增空表达式的跨后端语义。编译器追加的末尾签名复用现有 executable call metadata 标记来源，重检只删除带标记的签名，不再用定义是否属于 Snapshot 猜测来源。用户末尾 `hint-fn` 在严格与兼容模式的 native/JS 都保留；内部标记不能由 source 构造，也不改变列表值语义。低层 Rust 测试仅验证标记与 quote 的保留规则，不另建来源表或公开语法。

若终止分支在首次预处理阶段就要求对空 payload 做静态方法选择，例如 `(value .unwrap) .to-string`，尚不能先完成整个函数体再从 transfer 收集约束。当前仍明确报 `E_DYNAMIC_POSTFIX_METHOD`，不改成动态派发或授予方法权限。后续需要把词法递归约束求解前移到依赖它的静态选择之前，并与统一 typed-core 校验协同；不以逐个方法放行或新增检查阶段开关绕过。这个最小复现和 Ref 边界继续由原 issue 追踪。
