# WASM 具名函数别名复用已知调用目标

## 实际问题

0.31.0 类型强化原型把 frequencies 改成参数化 Map / Option 方法路径后，新增函数值测试在 native、JS、core WASM 通过，WASI 因 E_WASI_COMMAND_INDIRECT 拒绝。更早的原统计实现对同一测试也在 native 通过、WASI 拒绝，属于 #2179 已记录的 lowering 限制，而不是计数语义回退。

该问题不要求修改公共类型合同，也不依赖尚未合并的泛型证明或 frequencies 改写。因此独立从 main 实现，只携带具名别名 lowering、独立附带测试、宿主边界回归和支持说明，不混入原型的其他源码。

## 决策与边界

不可变 let 的右侧若已解析为实际具名定义，在原函数值求值和写入之后保存该目标；以新分配的 WASM local 槽位为键，不按源名字或相同 Fn 签名猜目标。遮蔽会使用新槽位，闭包捕获保留原槽位；非递归内联调用的新参数槽位也可保存实际具名实参，普通函数参数和 recur 的可变槽位不凭签名取得这项证据。函数值表示、保存与身份不变。Fn 对象的 def_ref 还须与实际编译目标的 arity/rest 形状一致，不把匿名闭包的错误引用当作已知目标。

已知 local 的普通调用、spread 及高阶 callback 解析复用原具名定义路径，继续由既有 arity、rest、specialization 和可达依赖检查决定能否 lowering。未知条件表达式产生的 callee 仍走原间接路径；实际可达的不支持宿主能力即使经别名和内联 callback 调用，也必须按原诊断拒绝，且不写出产物。不增加表层语法、API、诊断、操作名例外、Dynamic、unsafe 或另一套 proof。

## 验证安排

Calcit definition :tests 覆盖别名链、内层遮蔽后的恢复、闭包捕获、函数值逃逸后的身份，以及 rest / 字面量 spread 与参数单次求值顺序，使用同一运行器回放四后端。已有 WASI Component CLI 边界测试保留原直接宿主拒绝，并增加别名宿主拒绝与内层未知目标遮蔽外层已知目标；先确认前端类型检查通过，再核对后端错误和无产物。

四项原 core 回归在 native/WASI 的实际排除审计中全部通过，才移除它们各自的 WASI 排除：hint-fn 的已证明回调别名、rest 函数值、多参数函数值，以及 identity 的本地泛型函数值。原断言没有修改；sort-by、Set map 等其他未知 callee 限制没有由此宣称解决。完整命令与最终结果保存在 PR，而不是让排除删除或本记录代替实际回放。

函数参数中的一般已证明 callback、动态 callee 集合、逃逸闭包 ABI 等仍按 #2179 原范围继续，不以本次局部具名别名通过宣称整单完成。最终 HEAD review、CI 与精确 main 工作流仍是合并门槛；本记录不替代实际结果。

## 审查补充

高阶调用共享具名目标以后，静态目标解析必须同时验证调用点的实际参数数：普通 map 等为一个，fold 和 map-indexed 为两个。校验统一作用于 Fn、Import、Symbol 及已知 local，而不是保留旧的 Fn 专属二参数假定。非 rest local 不匹配时仍走原表间接调用，不生成签名错误的直接 Call；rest 继续复用原打包或缺参 trap。此处直接构造不兼容 backend 元数据的回归留在 Rust，因为完整 strict 应先拒绝这样的 Calcit 程序。合法别名行为继续由 definition :tests 覆盖，并在现有 Component smoke job 明确选择 fixture/tag 回放四后端。
