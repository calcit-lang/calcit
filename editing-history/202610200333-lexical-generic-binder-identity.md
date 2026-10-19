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

Review 还指出普通检查会接受 `Fn<T>(T) -> T` 的具体常量返回；最小 eval 确认旧候选会接受 `1`。随后在同一返回证明中覆盖具体、开放和未知 producer：已建立的词法 binder 不因 producer 没有自由变量就失去刚性，也不因分支合并成开放值而获得转换证据。在报错前复用原 expression/逐出口证明，允许合法空容器，而不是给空集合或具体函数加豁免。显式 Dynamic 返回及真正泛型用 Dynamic 实例化继续允许。原 CodeEntry where-bound 尚未 freshen 的路径不宣称已修复，不能在能力/身份分离前把它的 trait 描述符误当成另一个变量并新增假拒绝；#2121/#2188/#2190 仍保持开放。

`substitute_type_vars` 保留现有实例化语义；词法重命名通过同一个迭代替换实现的捕获避免模式屏蔽嵌套量词。私有 key 不进入 source schema、公开类型描述或正常诊断。

## CI 中暴露的既有 loop 合同

改写后校验复现了 `repeat` 的真实关系冲突：loop 自己重新声明 `T`，但追加的是捕获的外层 `x: T`；只有空 acc 初值无法证明这两个独立变量相同。移除该 loop 的重新声明，使参数/返回 hint 引用外层 `T`，保持同一 append/recur 程序、公开签名和原有断言，不增加 checker 豁免。还通过官方源码查询核对了其他 core 函数体泛型 hint：`interleave` 的局部量词由它自己的三个参数传递，body 不借外层元素冒充新变量，因此保留该声明，不机械删除所有 loop 泛型。

兼容模式的泛型拒绝测试放回已有 `check-strict-default.mjs`；普通 proof harness 不含兼容开关，不修改原默认严格门禁的排除规则。

具体/未知返回校验还暴露旧 `test-generics.main/test-fn-generics` 的 `id2` 合同不完整：它声明了泛型返回 T，却没有声明参数与 T 的关系。只补该函数的 `:args [T]`，保留其恒等函数体、Number/String 调用与全部原断言；没有从返回声明反向制造输入证据或默认给所有未注解参数套上泛型。同样的不完整合同作为负例进入原 harness。CLI 产生的 recursive-struct 排版折叠通过 program-diff 确认，不据此改写原语义。

## trait 能力与身份分离

完整词法返回检查使带 `T: Debug` 的合法局部恒等函数暴露假拒绝：参数早先被替换成 `trait Debug`，返回声明却仍是原 T。现在参数始终保留原 TypeVar；ScopeTypes 以 binder key 独立保存能力，只有该函数自己量化的变量可以引入假设。嵌套作用域共享不可变的 capability map，修改时 copy-on-write；普通无约束作用域不额外分配该 map。值、细化和能力的生命周期分开，闭包捕获及同名遮蔽不会制造新证据。

方法解析、origin-qualified trait lowering 和 where 校验读取同一能力证据，但普通值类型比较不把 trait 当作身份。CallTypeProof 显式接收调用者能力上下文，嵌套泛型回调实例化沿用它；回调自己声明的量词/where 不再被“无注解”推断丢弃。普通参数检查仍保留原未解决边界策略，但已知矛盾不能被兼容关系放行。显式 trait call 同样要求 receiver 能力或原有 default 实现，不再让无约束 TypeVar 绕过。

post-lowering 的原有遍历按函数 metadata 恢复能力作用域，不用空 ScopeTypes 复验合法转发；没有关闭门禁或加函数例外。type-at 的原 source trace 保存泛型局部及参数的能力，现有 static-methods 字段同时显示 T 与其方法，不新增 CLI 字段或入口；EDN/JSON 继续等价。返回自身的 `.slice` 方法仍显示返回 T，而不是 Sliceable。

曾试验同时切换全部 CodeEntry 的默认返回证明，发现 core destruct-set 的旧 Option<List<Dynamic>> 异构 pair 使 SetDestruct<T> 退化为 SetDestruct<Dynamic>。这是 #2121 全局切换前须修复的既有精度问题；本次没有增加 destruct-set 豁免，也没有提前启用该全局切换。完整 #2187/#2121 与公共泛型合同收尾仍不宣称完成。

库回归还定位到一个独立测试竞态：人为构造的 defimpl stack 用了受全局 TRACK_STACK 开关影响的 extend，而另一测试临时关闭了开关。仅将该固定 fixture 改为 push_left 明确的 frame，保留原断言，不改变 runtime 的全局开关或串行化所有测试。

## 验证方式

- 合法恒等、纯改名、未重新声明的外层捕获、三层同名遮蔽与外层恢复、Number/String 的嵌套 mapcat/map、顺序和空维度，使用 definition `:tests`，由原共享运行器执行 native/真实 JS。
- 新增空 List/Map/Set（包括空 Map 分支）和 Dynamic 实例化的恒等回调，分别以 Number/String/List 验证；具体 Number、错误元素 List、混合分支和开放 payload 的拒绝复用原同一 harness，核对源码不变、source owner 和拒绝产物。
- 原有 check-known-assertion 运行器覆盖错误捕获的 T/U、CodeEntry/body-hint 两种外层合同，以及 proof、完整 strict、普通/compat 检查、native 和 JS 编译拒绝；核对源码不变和拒绝时不产出应用模块。
- 低层 Rust 测试只验证内部 binder 重命名不捕获嵌套量词、checked metadata 保留身份、source schema 不泄漏 key，以及声明作用域与值/细化状态的独立生命周期；语言成功语义不重复写成 Rust 断言。
- 原 native/JS 主测试、原类型/调用证明回归、cargo fmt/Clippy/test、文档代码块和真实消费者查询另按 PR 记录实际结果。未运行或未通过的项不写成成功。
