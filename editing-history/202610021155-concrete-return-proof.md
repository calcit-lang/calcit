# 具体返回值的独立证明导航

为已有 fix 入口提供独立的 `concrete-return-proof-v1` 选择，复用 assertion audit 已实现的
共同 compiler proof、源码重处理与 compiled-state checkpoint，不添加平行类型关系；
使用既有 `E_FN_RETURN_UNPROVEN`，避免 wrapper 的声明成为 producer 实现的循环证明。

报告保持 source owner/path、fingerprint、origin 与非零退出。只允许所选项目 scope 内的导航；
在 scope 外发现 producer 时要求显式选择 owner。未解决的断言、权限、矛盾返回等其他错误
仍阻断审计，不因为选择返回规则而隐藏。

不从预期返回类型猜写 schema。缺少实现证据只能 review；只有独立证明的缺省 metadata
才能交给已有 synthesis 规则及其调用点/原子事务门禁。本阶段不翻转默认检查、不加入自动 preset，
也不宣称 callable、nominal-write 或完整 strict workflow 已交付。

共享语言行为继续由现有 return-boundary Calcit tests 与 native/JS/WASM runner 验证；
新增 Rust CLI 回归只检查导航、scope、EDN/JSON 报告与 apply 不修改 source 的工具边界。

自审复现了 `sink: Number -> Number` 被 `wrapper: Dynamic -> Number` 调用时的证明漏洞：
审计借用了 sink 返回声明，却没有验证传入的 Dynamic。显式 proof pass 现在复用现有调用参数检查
与 `prove_available_bindings`，覆盖具名函数、局部函数和 proc；普通编译的检查策略保持原有行为。
前缀和后缀方法调用复用 `static_method_contract` 的接收者实例化结果，并进入相同的参数证明关系，
防止 `.rem 3 dynamic-value` 经方法 lowering 绕过检查。
新增 `E_CALL_ARGUMENT_UNPROVEN`，因为既有泛型擦除诊断无法表达非泛型 Number 参数缺少证据，
确定类型矛盾的 warning 也不能代替未证明诊断。未知实参仍显示 unknown，不伪装为用户声明的 Dynamic。
审计与 schema synthesis 同时拒绝编译器已有的类型矛盾 warning，防止据此获得空报告或自动写回。

参数审计同时暴露了共享证明关系的方向性问题：具体 Fn 保存为开放 Fn 不需要凭空获得新能力，
应可证明；开放 Fn 用于具体签名则仍需边界证据。修正共同关系，并在 hint-fn 的 :tests 中加入
函数保存与 checked decoder 调用案例，由现有 runner 回放 native/JS 和显式 proof pass。

Respo 回归暴露 `cond` 宏实现中语法树操作的历史 warning 被误当作运行时返回矛盾。
warning 过滤依据定义的 Macro schema 区分编译期实现与展开后的运行时代码，和已有 proof pass 的
macro policy 保持一致；展开后的实参类型矛盾仍须失败。补充 cond 的 definition :tests 与 CLI 拒绝回归。

审查发现 `&` 展开调用绕过了参数证明。proof pass 对源列表字面量按位置展开证据，复用同一
类型关系；非字面量展开暂保留人工 review，不能借用 callee 返回声明证明输入。返回推断复用
同一无求值的字面量展开，运行时 AST 和求值顺序不变。显式 `&call-spread` 进入相同预处理路径。
新增 `E_CALL_ARGUMENT_MISMATCH` 表示已证明矛盾：既有 UNPROVEN 表示证据不足，不能把确定的
String/Number 冲突降为可导航的缺失证据；普通编译的 warning 策略不变。CLI 回归检查 source
不变与两种报告格式，正向调用语义放入 hint-fn 的 :tests 并通过既有 native/JS runner 回放。
