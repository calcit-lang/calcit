# Callable 契约审计与嵌套签名元数据

## 决策

关联 #1539。复用现有 argument proof 和 `fix` 的 compiler review adapter，增加显式规则选择；不另写 callable 类型关系，不增加顶层命令或诊断编号。裸 `Fn` 的保存与传递保持合法，进入具体签名才要求证明。

未知 callback 的签名不能从单个调用样本猜出，因此建议只供 review，replacement 为空。定位保留编译器提供的 source owner、参数路径、指纹和词法上下文；不宣称已经追踪跨 helper 的最早擦除位置。较早的非 callable 证明失败仍按共享编译器顺序报告。

## 本轮发现

新增 rest/features 回归发现，嵌套签名经过 `edn_type_to_calcit` 时，Set 和 Bool 被转换成 Nil，分别丢失 features 与 async。保留这两种元数据；源码形式的 literal feature set 也只读取字面 tag，不执行表达式。保留内部 async marker 的防伪规则；签名显示 async 状态，但 features 提示不暴露内部 marker。不改变词法 FFI 授权规则。

## 验证

进一步回放现有 fixed-callback 定义测试发现，局部具名 callable 的实参预处理没有注入已知 callback 参数类型；复用同一已解析签名补足上下文，不由 callee 返回声明猜测函数体。恢复 features 元数据后，新增负例又暴露 contextual signature 可错误授予匿名 callback 的 `:js-ffi`：所有调用点注入只保留类型约束，不携带 features/async，定义自身声明与词法父作用域仍由既有规则处理。未知 callback、权限缺失均不可用补签名或宽化类型绕过。

语义正例附加为 Calcit definition `:tests`，覆盖结构化 callback 调用与裸 Fn 保存。现有 CLI 测试覆盖缺证据定位、rest/features、EDN/JSON 输出、preview/apply 保持 bytes 不变；parser 回归覆盖嵌套签名 metadata roundtrip 与保留 marker 过滤。继续使用既有完整测试门禁，不新增运行器。

来源传播链、自动 metadata synthesis 与 strict workflow 的完整集成仍需按 #1539 后续验收，不因新增规则入口就宣称全部完成。

CI 的完整 core 回放还发现 `add-watch` 的 callback 收到未实例化的 `T`，而后续参数检查已从 `Ref<Number>` 推导出 Number。间接调用的预处理现在复用通用类型证明，从此前已经处理的实参逐步收集 bindings，再替换 callback 上下文；Dynamic 与没有元素证据的空集合不产生绑定。附加定义测试通过 watch 的新旧数值求和验证推导和运行结果，不靠手写 callback schema 掩盖问题。

Review 要求的泛型审计正反例进一步发现：泛型函数存入局部变量后，调用结果直接返回未替换的 `T`。局部与嵌套 callable 调用复用已有泛型返回推导，根据实参替换自身泛型；不是该 callable 自身声明的外层符号类型继续保留，避免破坏 generic helper 内部 callback 的 `T -> T` 关系。测试同时覆盖直接、局部、嵌套调用与擦除签名后的拒绝。
