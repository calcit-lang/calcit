# Enum empty? 的输入证明与静态方法调用

完整 trait fixture 的 strict workflow 暴露了 `&core-enum-methods` 中匿名
`&enum:empty?-impl` 的参数证明缺口：函数体调用 `&enum:count`，但参数 `x` 没有合同。
常规测试执行成功不能代替这个证明。给该内部函数补 `Fn(Enum) -> Bool`，保留原有
计数和比较表达式，不扩大 Dynamic，也不让 core 绕过检查。

新增的 Option/Result 定义测试还复现了已有 WASM/WASI 陷阱。对同一 main 分别保留和
移除注解后，`empty?` 都失败，单独 `count` 都通过。原 generic `empty?` 先调用接收者
的 `.empty?`，再检查返回值是不是 Bool；无类型的标量后端丢失了该方法的返回合同。

修复复用现有 impl 选择、参数证明、直接 callable 和改写后校验。已有轻量 primitive
specialization 仍优先；只有实际选中的方法能独立证明参数，并且返回 Bool 时，才把
公共 `empty?` 改写为那个直接调用。不会按方法名字假定结果是 Bool，也不会用
`&enum:count` 替换用户自定义方法。缺失、歧义或未证明的合同保留原 dispatcher 和
运行时 Bool 检查；生成 callable 的来源、冲突和缓存依赖继续走现有机制。

语义测试放在 Calcit `:tests`：core 的两个用例覆盖 Option/Result 的 tag-inclusive count
与 `empty?` 结果，并用同一运行器在 native、JS、WASM 和真实 WASI 执行。现有 trait
fixture 再覆盖接收者只求值一次、输入异常仍传播，以及自定义非 Bool 方法和 nil 仍被
拒绝。这两项使用原 native/JS fixture 的 ref/try 能力，不宣称 WASM 支持这些构造。
原测试、断言和后端排除均不删除或放宽。

现有 strict-default 脚本增加针对实际 core impl 的限定范围 callable proof，避免仅凭
运行结果掩盖输入证据缺失。完整原 trait fixture 到达下一处内部 Enum contains? 参数
证明缺口时，仍如实保留失败并另行跟踪，不把本项局部通过写成全项目 strict 通过。
