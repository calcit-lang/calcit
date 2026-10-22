# Struct 定义值的语义身份

## 决策依据

Struct 定义值的相等不只是名称与字段名。native 已比较声明来源、字段类型、泛型参数、约束和 impl；JS 原来只比较名称与字段名，却把字段类型和 impl 放入 hash。真实消费者因此把 EDN 恢复的旧定义当成当前定义，也可能无法用相等定义查询 Map。

本次保持 native 的定义值合同，在现有 JS 类型形式模块中投影相同的语义元数据。原始形式保留给诊断和字段检查；primitive 别名、容器默认参数、Fn schema 默认值以及泛型约束先规范化，Eq、hash 和 ordering 共享投影结果。构造、绑定、别名和 impl 装饰保留这些元数据。生成的 validator 闭包只做运行时检查，其地址不属于类型身份。

两个模块的局部 Struct 可以具有相同名称和字段名，但其字段中的同名 nominal 类型并不是同一来源。引用到 JS 时复用 native 已有的类型名解析，给 quoted symbol 附带可选来源；普通 Symbol 的打印、相等、hash 和 ordering 不受该元数据影响。类型投影尊重声明的泛型遮蔽，不能把同名泛型替换为模块定义。

新增 Calcit 附带测试还暴露了 native 类型排序仅使用简略名称、或漏掉回调 rest / features 的问题。因此复用现有 Trait 与完整 Fn comparator，确保不相等的类型证据不会排序为 0；只在 Calcit 无法构造的未求值 Trait schema placeholder 上保留一个 Rust 内部断言。

## 边界

不改变 Struct 实例相等、类型兼容性或 Dynamic 的准入规则，不把定义值相等当作类型证明。EDN 不携带完整运行时来源，恢复后是否等于当前定义取决于实际元数据；开放匿名定义仍保留合法相等的情况。新逻辑不引入 checker、固定 fix、迁移白名单或额外诊断。

## 验证

通过官方 query / edit ns 与 dry-run、scoped revision 守卫把测试放入原有 trait Snapshot，复用统一 native/JS 运行器。覆盖字段合同、来源、泛型、约束、回调、EDN、impl 装饰、等价写法和 Map/Set；现有 JS host harness 验证 validator 闭包不参与相等、普通 quoted Symbol 仍相等，以及 SliceMap / TreeMap 使用一致的 key 身份。保留原核心、严格类型、Struct 写入与 WASM 回归，不新增测试排除。具体执行结果、真实消费者验证和 CI 状态以 PR 为准。
