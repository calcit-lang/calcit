# 显式 trait 调用保留实际实现

## 问题与选择

已知名义 Enum 的普通方法调用已能保留数值操作数证据，但 `&trait-call Contains :contains?` 的合法 Number 调用仍会在 WASM 函数表冻结后尝试生成新的 callable，随后报目标函数不存在。补丁直接使用已检查、无捕获的实际 impl 函数体，复用普通方法的内联和递归保护；有稳定函数表入口时仍使用该入口。

名义数据合同只证明字段与变体，不证明附加的 impl。显式 trait 调用继续按 trait 来源选择唯一实现，不能按方法名重新分派，也不能从同一名义类型推断默认实现。已有调用点特化现在同时覆盖显式 trait 调用，并携带实际值的实现身份；无法证明身份的开放边界保持明确拒绝。

不修改表层类型关系、数值比较规则、公共 API、诊断编号或后端排除表。接收者为单独 `raise` 的写法在原有前端就无法证明 trait 接收者，不能把它算成本次后端回归；失败顺序测试使用类型可证明、运行时抛错的条件接收者。

## 回归与验证

- 在原 `test-explicit-trait-call` 的 `:tests` 增加 Number 范围、显式来源、同数据类型的不同 impl、泛型约束、工厂和 identity 返回、有限嵌套调用，复用统一四后端运行器及现有 portable tag。
- 在原 WASM fixture 和宿主脚本增加显式调用的非法 key、真实 heap-pointer 数值别名及失败前单次顺序求值。宿主只验证 WASM 无法自行捕获的 trap 和输出轨迹，不复制测试运行器。
- Snapshot 通过官方 query、受 revision 保护的事务和完整 readback 修改；program-diff 确认只新增测试定义及附带测试，没有删除原用例或放宽 schema。
- 复用原完整 Cargo、Clippy、check-all 与 core 四后端回归；额外将实际非法 key/异常用例回放到 native、JS 和真实 WASI，核对错误和输出轨迹。
