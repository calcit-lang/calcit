# frequencies 的私有计数类型流

完整 strict 在计数递增的回调参数上丢失 Number 证据；普通执行得到正确结果并不能替代证明。
修复仅在私有空累加 Map 上声明 `Map<T,Number>` 上下文，让现有 receiver-specific
update 合同把 Number 传入回调。

这不是对开放输入的强转：空 Map 没有待验证的旧 payload，首次写入为 Number 字面量1，
后续更新继续由既有 Map 写入和回调检查约束。泛型 T 来自输入 List；没有把业务 key
转成 String，也没有通过函数返回声明反向证明回调输入。

原遍历、contains 分支、update/assoc、求值顺序和计数算法不变；不修改编译器证明规则、
诊断、Dynamic 策略或公开 API。

用户可观察的重复项、空输入、nil/Tag/List/Map key 语义保留在 core definition 的测试，
由已有 native/生成 JS runner 回放。现有 CLI 集成测试负责完整 strict 的模块无关 caller
和验证不写回的进程边界，不在 Rust 重复实现计数语义。
