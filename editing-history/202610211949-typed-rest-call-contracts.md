# rest 参数与展开调用共用既有函数合同

## 证据与范围

外部 trait 方法只比较固定 `arg_types.len()`，忽略声明的 `rest_type`，因此真实 React
可变 children 调用连一个合法 String 实参也会被判为多余参数。追查同一关系时，发现普通
具名函数和局部函数会跳过非字面量 spread 的元素检查：`List<Number>` 可以进入
`rest String`，随后 native 的 String 使用才失败；现有独立 callable proof 已能拒绝该调用。

修复共用参数关系，不引入 React 例外、checker、诊断编号或固定迁移规则。普通调用补齐的
已知类型拒绝属于 0.31.0 的非 patch 范围，不能仅因原方法问题列在补丁队列而随补丁偷偷收紧。
这只是普通检查与 proof 对齐的一部分，不代表总单或整个生态验证完成。

## 设计

- 复用单个末尾 spread 的 proof-only 投影：必需前缀已经齐全时，把展开列表与
  `List<rest>` 比较；显式 rest 实参继续逐元素检查。
- trait 方法使用已有 callable arity 和普通参数检查循环；保留泛型、where 约束及源码位置。
- 返回推导和现有 spread 证明读取同一份选中方法签名。`&call-spread` 包装不能抹掉
  方法的返回证据，也不能让显式 core 写法避开普通检查。
- 投影只供证明，不改写执行参数。保留原生方法调用及 JS receiver，不将测试换成 native call。
- 不带 spread 的方法推导保留借用路径，不为所有普通方法调用复制参数树和完整签名。

## 验证方式

正常语义放在定义自身的 `:tests`：普通函数覆盖空 rest、显式实参、空/非空 List、
显式实参加末尾 List 和单次求值，并由原运行器共享 native/JS 回放；外部宿主方法的
测试也写在 Calcit 中，通过已有 JS FFI harness 在真实 JS 对象上回放。

编译拒绝使用原 harness：局部/具名函数以及显式 `&call-spread` 检查 Number 列表的
精确 expected/actual 与 source owner；JS harness 拒绝直接 Number、Number 列表和
缺少必需前缀，并验证完整 strict workflow 的结论。原 core、type-fail、spread、
strict-default、JS FFI 回归及 Rust lint 继续保留。

宿主执行验收只证明 JS。真实 Expo 应用仍需使用发布后的 CLI/runtime 配对进行自己的
Hook、Metro/Hermes 与移动端导出验收；本地未发布候选不能冒充正式消费者配对。

## 审查与 CI 补充

初轮 CI 的两个 fix 协议测试仍期待开放值进入具体 Number 合同后能预览为待审项；当前普通
检查已经提前拒绝这些调用。保留全部用例与文件不写入断言，将对应开放输入明确改为断言
`E_CALL_ARGUMENT_UNPROVEN`、精确源码位置和普通检查同样失败；没有回退检查或启用兼容模式。

审查指出无法整体投影的 spread 会跳过已知前缀，以及显式 `&trait-call` 的开放传输签名
会掩盖选中方法合同。前缀改用同一参数循环检查，但不把局部检查当成完整 where 证明；
显式调用先保留 trait 身份、方法和实现验证，再按同一展开合同检查 arity、参数与返回值。
不能证明的展开报告已有参数诊断，不误报为缺少运行实现。

新增 `RestBox` 的 native/JS 定义测试还发现运行时 Fn 元数据把 rest List 的绑定计入了
最少实参数，导致相同表面签名的 trait 实现被 native 拒绝。修正 `from_calcit_fn` 的数量
计算，不改变执行参数绑定；普通名义方法也复用共享 rest 检查，删掉旧的逐项 Variadic 检查。
合法 rest、必需前缀、直接/显式方法与只求值一次均保留行为回放。

类型证明不伪造 source identity：显式原语缺少唯一 source-call 映射时，既有 spread 改写
仍可要求审阅，不能因此猜一个可写回的替换。该限制不阻止合法调用通过独立参数/返回证明或执行。
