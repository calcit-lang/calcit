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
