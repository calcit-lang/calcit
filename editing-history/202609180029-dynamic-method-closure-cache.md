# Dynamic methods 入口依赖闭包缓存

- `analyze dynamic-methods --incremental` 复用现有 Cirru EDN input/analysis cache，不增加新的顶层工具入口。
- 缓存键由活动 entry 的 init/reload roots、compiler-resolved dependency closure、definition/namespace revision、entry policy 与 Calcit 版本共同决定。
- 只持久化动态方法诊断，不序列化 compiled AST；依赖记录缺失或未解析时保守执行完整入口预处理。
- 回归覆盖 cold/warm、闭包外 definition 变化继续命中、入口 root 变化失效、诊断缓存字段往返，以及增量/无缓存报告一致性。
- Respo 临时副本上的 `--summary-only` 实测由 cold 1.46s 降至 warm 0.42–0.43s；warm 报告为 1116 dependency hits、3910 edges、`preprocessing_cached=true`。
