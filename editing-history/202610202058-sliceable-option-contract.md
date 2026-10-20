# Sliceable 使用明确的 Option 终点

维护者在 #2213 确认将 slice 函数及 Sliceable.slice 的终点统一为 Option<Number>。
显式终点使用 Option :some end，省略或 Option :none 表示到末尾；不新增公共入口，
不使用 nil 转换或附加 Countable 约束。原 slice 的问号参数和编译器中的该项例外删除。

List/String 的普通 method 表优先于 trait impl 表，因此两处都必须指向同一内部适配器。
适配器匹配 Option 后调用原数字原语；原语本身的范围错误、Unicode 单位和求值行为不变。
drop/take/take-last、原附带测试和示例、测试模块中的显式终点均用受 revision 保护的事务迁移。
不改原断言结果；不新增永久 checker 或固定 fix 规则。

自定义 Sliceable 的回放发现 &trait-call 推断把已求值实现与新的源码 trait 占位值比较，
错误得到零个匹配而丢失返回类型。查询已有 Ready trait，保留其运行时名义身份；没有 Ready 值时
仍走原源码恢复，不执行新的程序初始化。同名不同来源和不同运行时身份不因此合并。
该修复通过 Calcit 定义附带测试表达，不用额外的断言、Dynamic 或 unsafe 补洞。

完整既有拒绝 harness 随后揭示 bounded-local-map 的回归：声明中的 where 能力仍是源码合同，
不能与已求值 trait 的运行时分配身份作相等比较。因此声明能力继续解析同一源码 origin，
具体 receiver 的 impl 查找才使用 Ready 值；不改变名义相等关系，也不按短名合并不同来源。
原有 30 项 lexical-binder 测试在 native/JS 全通过，不替换其声明或返回断言。

WASM 的旧 slice 专用分支为旧问号参数设计，将终点当成数字，不适用于新的 Option 合同。
删除该专用路径并尝试复用通用 trait 单态化；同时移除未知 slice 方法静默返回 nil 的旧 fallback。
实际回放暴露三个缺口：省略的尾部 Option 没有补入单态化实参，泛型函数作为局部值时在获得
调用参数前就被要求物化，自定义实现的 struct-with-at 尚不支持。固定参数的 Option 缺省先补入
真实名义 None 构造；已知且不可变的泛型函数别名留在编译器侧，调用时根据实际参数特化，
不捕获调用者的词法绑定，不尝试物化未知函数。闭包参数同样区分 Option 缺省与旧问号 nil。
struct-with-at 复用已有 Struct 更新的布局、种类和字段身份验证，先按顺序求值全部输入一次，
再进行不可变更新，保留原 receiver 的名义身份。没有删除函数值或自定义实现测试、增加排除项。

截至 v11/v12 本地候选，五项 core slice 测试和四项自定义实现测试均在 native/JS/WASM/WASI
各自全部通过，无排除。v11 的 Rust library 1030 项和 all-target Clippy 通过；新增 Struct 更新
支持后的完整回归仍待完成。七个错误输入在五入口均拒绝：显式 trait 调用复用已有合同参数
检查，不再因为返回值未使用而漏检；方法错误复用已有 source 回溯定位，不要求用户追踪 gen%。
完整既有拒绝 harness、全部文档/语义回归、真实生态消费者与附带/宏生成调用迁移、最终
CI/review 及精确 main 门槛仍未完成。v14 的 78 份文档／416 个代码块、原 native 与实际生成 JS
入口、历史 API 合同检查和 Clippy 已通过；完整既有拒绝 harness 仍在执行。正式消费者盘点先
取默认分支而非本地旧 checkout：Respo 主 Snapshot 的 402 个定义、143 项测试、74 个示例中，
6 处显式终点的事务预览成功，2 处省略终点保留，尚未应用；std 主 Snapshot 的 67 个定义、
44 项测试、32 个示例没有相关调用。独立 fixture、架构数据、依赖和其他消费者仍不在这次
盘点范围，不能当作零命中。这里只记录独立本地分支的工作，不表示 #2213 或 0.31.0 完成。

后续先 rebase 到已合并 #2220 的 main，保留其名义身份拒绝区块和本项 Sliceable 区块。
旧 v14 debug 完整回归仍活跃时，因基线更新有意停止，不记为失败或完整通过；不修改运行中的
输入。普通内联闭包的 trailing Option 也应补入名义 None，因此把已在独立 Snapshot 四后端
通过的同一表达式加入 slice 的 definition :tests，再以 release 候选重新回放完整门禁。

rebase 后的 v15 release 候选已重新通过 slice/helper 九项及自定义实现四项的四后端回放；
原 lexical-binder 三十项、局部类型别名三项在 native/JS 通过，声明命名空间三项在四后端通过。
完整 core Snapshot 的 686 项 native 测试、1030 项 Rust library、all-target Clippy、TS 检查、
78 份文档的 416 个代码块及历史 API 合同检查通过。原 native 入口与实际生成的 app.main JS
入口也通过；JS codegen 不生成 main.mjs，测试直接导入生成 namespace 并调用原 main!。
完整既有拒绝 harness 继续使用同一个 v15 进程，不因观察超时重启；结果待其正常结束。

v15 的完整既有拒绝 harness 随后正常结束、exit 0。正式 post-lowering CI 则暴露旧的
check-typed-string-conversions 用例仍要求所有泛型函数局部别名被 WASM 拒绝。单独回放确认：
具体 Number 实参的 to-string 别名已通过通用调用点特化并真实返回正确结果；开放泛型 receiver
仍以 E_WASM_TRAIT_TYPE_EVIDENCE 拒绝。这是本次已实现能力，不是放行未知函数表调用。
原用例保留同一表达式，改为检查实际 WASM 执行结果；追加开放泛型别名的编译拒绝和无产物
断言，保留原开放直接调用与 spread 的拒绝门禁。支持矩阵同步说明具体证据与未知边界。

CodeRabbit 对首个 PR HEAD 发现真实缺陷：仅为尾部 Option 补值也进入完整 body 内联，导致
已有 recur 或递归函数被当成递归特化而拒绝。新增两项 Calcit 附带测试在修复前证实 native/JS
通过、WASM/WASI 编译失败；分别保留 recur 累加与递归调用内部省略 Option 的正常语义。
补值不需要闭包、nil 或 trait 特化时，改为把完整实参传给原已编译函数；其他特化路径不变。

v16 的新增递归两项与自定义 Sliceable 四项在四后端全部通过。完整 core 回放最初有七项 JS
解构失败，定位为本地 lib 仍是旧生成物（TS 源码已使用 MapDestruct/SetDestruct，lib 仍返回
Option）；重新实际编译 TS 后，同一 v16 回放为 native686、JS685、WASM439、WASI434 全部通过，
沿用原排除清单。旧 v15 的逐项排除审计已因候选更新有意停止，不计为完整通过。

单独开启 CALCIT_LINT_CORE 的完整拒绝集继续暴露导入方法的漏检：method-inline 返回的 Import
调用未先走普通函数参数检查，后置校验因此对错误 Option payload 报内部错误。复用与后置
校验相同的已编译函数解析和 check_user_fn_arg_types，在返回已补全实参的 lowered call 前检查；
不编译或执行新的定义，不改错误匹配、不关闭 invariant。原七个错误调用继续作为拒绝回归。

随后完整 Rust CLI 测试揭示上述追加检查造成 List.apply 重复诊断，原断言的数量从 1 变 2。
这证明此前漏检定位不准确：原方法检查已经拒绝错误，丢失的是 literal-only 调用的 source
坐标，而非实际类型检查。因此删除该次追加检查，不改原数量断言；方法 lowering 在记录证据时
保留 source 位置，后置校验仅对没有 operand 位置的调用使用这一对应。相同 lowered AST 若对应
多个 source occurrence，必须全部具有已报告的检查，不能凭其中一个遮蔽另一个。内部 metadata
invariant 测试覆盖无证据、无关位置、相同 AST 的部分/全部报告；此层低级条件无法直接通过
Calcit 用户接口构造，用户可见拒绝继续使用原 Calcit harness。未关闭任何校验或增加诊断规则。
