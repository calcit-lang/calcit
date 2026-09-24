# 可选参数迁移的首阶段诊断

## 决策

`E_LEGACY_OPTIONAL_PARAM` 涉及省略、显式 `nil`、函数值、macro 与跨模块调用。现有 `fix` 的 revision、staged validation 和原子事务可供后续安全改写复用，但仅凭源码中的 `?` 标记与声明 schema，不能证明调用点和函数体语义不变。因此先在已有 `fix --rule` 下加入 `optional-parameters-v1`，只输出需要审阅的参数证据，不生成写入操作，也不加入默认 preset。

## 边界

规则读取未展开的 Snapshot 源码，显示参数位置、已声明类型和有依据时的 `Option<T>` 候选；`Dynamic` 不冒充可推断的业务类型。`--apply` 明确拒绝。后续只有把函数体判断、所有可写调用的解析与求值语义、macro/quote/外部消费者阻断都纳入同一事务，才能把无歧义子集升级为 machine-applicable。Calcit 行为测试及 Timegrass 实际迁移仍是 #1286 未完成的验收。
