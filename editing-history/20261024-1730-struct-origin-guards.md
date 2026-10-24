# Struct 来源守卫与名义证据保持一致

## 问题与决策

`&struct:matches? value Def` 的成功分支已被预处理器用于证明非泛型顶层 Struct
的完整名义类型，但 native 与 JS 运行时原来只比较短名称和字段名。不同 namespace
分别声明 `Box(:value Number)` 与 `Box(:value String)` 时，外来的 String Box 可以通过
guard。普通检查、独立返回证明和完整 strict 均接受的 Number reader，最终在 JS 返回
String `wrong1`；native 则直到数值使用处才失败。

修复运行时事实，而不是删除有用的来源推导或为 reader 增加特例：

- 目标具有来源时，native 复用已有 `same_nominal_definition` 数据合同关系。
- JS 把原 StructDef 相等判断中的数据合同部分提取为 `sameDataSchema`，供相等比较和
  来源判断共享。完整定义值相等仍另行比较 impl；附加 impl 不改变来源判断。
- JS 宿主对象的实际名称/字段布局检查也必须保留：不能仅因传入的 structRef 合同匹配，
  就接受其 header/layout 与该定义不一致的实例。普通 Calcit 构造器不会制造这种状态。
- 目标没有来源时保留原名称/布局匹配，但这种结构匹配不证明名义类型或字段 payload。
- 不改编译器证明关系，不增加 predicate、checker、Dynamic、unsafe 或固定改写规则。

## 实际调用与兼容边界

已知来源的别名、字段更新、附加 impl 以及同来源同合同的重新求值继续匹配。
不同来源即使名称/字段相同也不匹配；同 owner 改变字段布局或声明合同后，旧值不能
证明新合同。后者通过现有 Rust/JS 宿主测试验证，因为普通 Calcit 构造器不能制造这种
历史宿主状态。guard 不负责把未经校验的宿主 payload 解码成具体类型。

无 owner 的 EDN Struct 不会自动恢复顶层定义的来源。需要恢复类型的调用者应显式走
已有 Map decoder，并处理失败；不能由编译器替业务选择默认值或补强转。泛型定义匹配
只确认定义族，不绑定具体类型实参。局部 pattern 的结构匹配也不允许直接读取具体类型字段。

trait fixture 原来的 `GreetingImpl` / `LabeledImpl` 使用未证明的接收者断言。现改用
`struct?` 和来源守卫后读取字段，同时保留 trait 原本的开放接收者合同和普通方法调用。
先声明数据定义 `GreeterBase`，再用现有 `impl-traits` 形成 `Greeter` 别名，避免数据定义
与 impl closure 相互引用造成 JS eager initialization 环；打印名称仍是 Greeter。
这不是对任意循环初始化问题的编译器修复。

## WASM/WASI 的同类布局错误

实际回放发现，WASM 原实现把来源判断的第二个参数一律当实例指针，读取偏移 8。
但 StructDef 的表示本来就是模块内定义 ID，因此同源判断也会错误返回 false。

复用已有类型证据区分 ID 与实例指针，两参数仍按源码顺序各求值一次。具名目标比较
现有模块内名义 ID；登记的无来源目标按名称/布局匹配。无法证明第二个参数的表示时，
使用现有 `E_WASM_STRUCT_IDENTITY` 拒绝，不猜指针或返回默认 false。
WASM 模块仍不支持运行时创建局部 Struct 定义，不据此宣称支持动态热重载或所有 trait。

## 验证方式

- 现有 trait definition `:tests` 覆盖合法定义/实例目标、跨来源同名同字段、外来 String
  payload、别名/字段更新/impl、EDN 往返、局部与泛型 pattern，以及两个 impl 的错误接收者。
- 原运行器在 native/JS 回放完整 fixture；`:struct-origin-portable` 额外回放具名定义、
  实例、泛型族、参数目标与 `struct-match` 的五项测试。WASM 的选择进入原 `check-core-tests`；WASI 用
  同一运行器和真实 Wasmtime 验证，不增加另一套测试骨架或排除旧测试。
- 相同的九项回归在旧 native/JS runtime 各 4/9，通过新 runtime 各 9/9。
  后续新增的参数表示用例单独在四端验证，不把它算进先前九项红绿结果。
- 原 `check-known-assertion.mjs` 验证两个真实 impl 和 reader 的独立证明，保留字节守卫；
  两个反例要求局部结构/泛型族 guard 之后的 Number 使用仍然拒绝。
- 原宿主测试验证同 owner 的合同变更、缺失 owner、validator 地址与 impl 装饰。
- 继续执行原 Cargo、Clippy、文档示例和完整 post-lowering `check-all`；局部回放不代替
  最终 HEAD 的实际 review、完整 CI、CLEAN 和合并后的精确 main 验证。

用户规则见 `docs/features/structs.md`、升级步骤见 `docs/run/upgrade.md`，WASM 的类型
证据与 unsupported 边界见现有支持矩阵。本记录不代表其他 core/生态类型债务已完成。
