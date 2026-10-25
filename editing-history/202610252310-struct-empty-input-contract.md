# Struct empty? 内部输入合同

## 问题与决策

Enum 内部 callback 的输入合同补齐后，原 trait fixture 的完整 strict 到达
`calcit.core/&core-struct-methods`。其中 `&struct:empty?-impl` 调用
`&struct:count x`，但匿名函数自身没有声明 `x` 的 Struct 输入合同，独立检查报告
`E_CALL_ARGUMENT_UNPROVEN`。方法所属的 impl 不能替代函数体自身的输入证据。

在原函数体前加入 `hint-fn`，声明 `Fn(Struct) -> Bool`。原来的 count、与零比较、
求值及失败路径均不改动；没有新增类型推断规则、诊断编号、checker、公开入口或
Dynamic/unsafe 边界。现有 strict 脚本复用同一 scoped proof 检查 Enum 与 Struct
的三个内部实现定义。

Snapshot 修改通过官方事务的 dry-run、scoped revision 守卫和完整 query readback
完成；整体 program-diff 没有 schema widening 或定义删除。原 trait 测试及其断言保留，
新增定义附带测试而不是 Rust 侧的间接运行时测试。

## 验证与未解决边界

限定 Struct callback 的 callable proof 从失败变为成功。新增测试覆盖空 Struct、
带字段 Struct 的 `.empty?`、`empty?`、公开 count 与显式 Countable 调用，以及经过
开放 Struct 参数时接收者副作用的实际执行次数。

空／非空的名义值测试在 native、JS、WASM 和真实 WASI 全部通过；副作用测试在
native/JS 通过，但开放 Struct 参数调用 `empty?` 在 WASM/WASI 仍于首次输出后 trap。
该失败用例继续保存在 `:tests`，没有改成 native call、删除调用或新增后端排除。
输入合同修复不等于开放方法分发已完成，后者仍需依据实际选中的方法实现保留返回证据，
不能只因方法名为 `empty?` 就假定任意结果为 Bool。

完整 trait strict 已越过本次 Struct 输入缺口，但仍报告内置初始化函数
`&init-builtin-impls!` 的 Unit 与 Optional<Unit> 返回证明不匹配；不能把局部 proof
通过写成完整 strict 通过。本次没有修改该初始化函数，也不宣称修复公开 Enum 分发。

原 core 四后端回放保持通过，既有排除不变。完整回归及精确提交 CI 的终态证据放在 PR，
这些已发现的失败和后续验收继续由 issue 追踪，不以本内部合同补充提前关闭。
