# 公开 contains? 只对名义接收者静态选择实现

#2381 为修 #2339，把 Enum 的 `.contains?` 默认实现提成具名 core 定义，并把公开
`contains?` 在 Enum/Struct 接收者（含开放 `'Enum`）上改写为直接调用选中的实现。
review 实测发现回退：`'Enum` 只证明值属于 Enum，不能证明运行时用的是 core 默认实现。
带用户 impl 的名义值传给 `Fn(Enum) -> Bool` 包装后，公开 `contains?` 被静态绑到默认
范围判断；具名默认实现还让普通 `.contains?` 方法内联对开放接收者走了同一条路，
用户实现返回的 false 变成 true。

修正：

- 恢复 #2379 的内联默认实现（`Fn(Enum, K) -> Bool` 合同不变），开放接收者的普通方法
  调用重新走运行时分派。
- 公开 `contains?` 的改写只接受名义 Enum/Struct 接收者，它们的实现在类型上可静态确定，
  与 `empty?` 的做法一致；开放 `'Enum` 保留原 dispatcher 与运行时 Bool 检查。

回归测试：core `contains?#public-nominal-enum-indices` 在 native/JS/WASM 覆盖名义 Option 的
范围语义；`test-traits` 的 `contains-keeps-open-enum-impl` 用 review 的复现覆盖显式 trait-call、
开放 `'Enum` 包装的方法调用与公开调用（native/JS）。

#2339 中开放 `'Enum` 公开调用在 WASM 的陷阱重新开放：需要实际调用点的实现证据，
或让 WASM 的运行时分派保留结果标签，不能只凭 Enum kind 选 core 默认方法。
