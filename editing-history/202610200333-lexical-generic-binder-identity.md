# 局部泛型的词法身份与函数元数据

关联：#2190、#2188、#2121、#1553。目标版本为 0.31.0，不增加 0.30.1 发布前置。

## 问题与决定

局部 Map hint 重新声明 `T` 时，原表示仅保存名称，内层参数与外层捕获会被当作同一个类型变量。同一错误只把内层改名为 `U`，严格证明就会改变结论。声明不能为实际返回值提供这种转换证据。

为局部声明分配私有 alpha-renamed binder key，源码名称只用于解析与公开显示。解析作用域从内向外查找，未重新声明的闭包沿用捕获者的 key。只重命名自由变量；嵌套函数自己的量词屏蔽同名外层替换，继续使用迭代、带作用域的替换缓存，不用深递归处理嵌套类型。

三层闭包的追加复现表明，不能从所有可见值收集 TypeVar 名称后按排序推测词法遮蔽：外层和内层的值可以同时可见，第三层应引用最近的声明。现在由 ScopeTypes 独立保存声明的名称到 binder key 的映射，函数声明进入时绑定、嵌套作用域复制、顶层源码定义重置；值插入、细化和被传递的类型变量不引入新的词法声明。CodeEntry 中声明的泛型也先进入本函数作用域，body hint 可引用同一声明而不重复建立 binder。

检查后的函数 hint 在已有可执行 List 元数据中保留完整签名。source AST 和打印的 schema 仍只包含源码名字，推断和 runtime 取签名时不再通过序列化后的名字重建绑定身份。普通 List 的值相等、排序和 hash 不因该内部证据改变；重建源码仍丢弃派生元数据。

方法合同内部继续保留完整 binder key，不能提前用显示名称替换语义键。对 `interleave` 局部 List 的真实 type-at 查询进一步复现了 static-methods.generics 泄漏 key；修复仅放在 CLI 的 ContextMethod 投影边界，并通过原 Agent 接口脚本覆盖 EDN/JSON 一致性及 human 输出，不改变内部方法实例化关系。

原 Agent 接口回归还发现，直接保留签名会跳过旧重解析中的名义类型限定，使 Option 方法查询从 `calcit.core/Option` 退回无命名空间的 `Option`。在生成 checked hint 时复用原 namespace resolver，同时保存声明来源和 binder 身份，不修改原查询断言。两个普通调用/方法诊断也在输出边界还原源码名字，内部关系不变。

独立泛型返回继续复用 CallTypeProof 的刚性关系。对于已建立词法身份的返回声明，普通预处理同样不得把另一个词法变量绑定为它；未绑定的上下文求解变量不因此变成刚性 binder。未新增诊断编号、改写规则、入口、unsafe、Dynamic 或函数名白名单。

## 范围与尚未完成的关系

本次先修复局部函数声明和捕获，以及完整签名重建造成的嵌套回调证据丢失。没有宣称完成 #2190 的全部 trait-bound 验收或 #2188。CodeEntry 的 where-bound 身份与能力仍按原实现处理；把它们一并 freshen 的早期试验触发了 core trait 初始化中的 Mappable 依赖环，因此未开启该改动。下一阶段须分离能力和泛型身份，不以同 trait 等同于同一变量，也不跳过 core 初始化来绕开问题。

Review 还指出普通检查会接受 `Fn<T>(T) -> T` 的具体常量返回；本地最小 eval 确认当前仍会接受 `1`。本 PR 的刚性扩展只处理有词法变量的 producer，不宣称完整解决具体/开放 producer 的返回证明。具体返回、空容器和 Dynamic 的逐出口关系由 #2121/#2190 后续统一处理，避免用局部特例改变现有开放边界。

`substitute_type_vars` 保留现有实例化语义；词法重命名通过同一个迭代替换实现的捕获避免模式屏蔽嵌套量词。私有 key 不进入 source schema、公开类型描述或正常诊断。

## CI 中暴露的既有 loop 合同

改写后校验复现了 `repeat` 的真实关系冲突：loop 自己重新声明 `T`，但追加的是捕获的外层 `x: T`；只有空 acc 初值无法证明这两个独立变量相同。移除该 loop 的重新声明，使参数/返回 hint 引用外层 `T`，保持同一 append/recur 程序、公开签名和原有断言，不增加 checker 豁免。还通过官方源码查询核对了其他 core 函数体泛型 hint：`interleave` 的局部量词由它自己的三个参数传递，body 不借外层元素冒充新变量，因此保留该声明，不机械删除所有 loop 泛型。

兼容模式的泛型拒绝测试放回已有 `check-strict-default.mjs`；普通 proof harness 不含兼容开关，不修改原默认严格门禁的排除规则。

## 验证方式

- 合法恒等、纯改名、未重新声明的外层捕获、三层同名遮蔽与外层恢复、Number/String 的嵌套 mapcat/map、顺序和空维度，使用 definition `:tests`，由原共享运行器执行 native/真实 JS。
- 原有 check-known-assertion 运行器覆盖错误捕获的 T/U、CodeEntry/body-hint 两种外层合同，以及 proof、完整 strict、普通/compat 检查、native 和 JS 编译拒绝；核对源码不变和拒绝时不产出应用模块。
- 低层 Rust 测试只验证内部 binder 重命名不捕获嵌套量词、checked metadata 保留身份、source schema 不泄漏 key，以及声明作用域与值/细化状态的独立生命周期；语言成功语义不重复写成 Rust 断言。
- 原 native/JS 主测试、原类型/调用证明回归、cargo fmt/Clippy/test、文档代码块和真实消费者查询另按 PR 记录实际结果。未运行或未通过的项不写成成功。
