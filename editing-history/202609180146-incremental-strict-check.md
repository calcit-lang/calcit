# 增量复用成功的 entry 严格预处理

`calcit <snapshot> --check-only --incremental` 复用已有的 module input cache 与 compiler-resolved dependency closure。缓存只记录在相同 strict policy、dynamic-method policy 和 closure revision 下已经成功的检查，不序列化 compiled AST，也不缓存 warning 或 error。

闭包外 definition 变化可以继续命中；reachable definition/schema、namespace import、entry type-slot、feature policy、编译器/core 输入变化会失效。模块加载失败或闭包证据不完整时仍执行保守路径。失败检查不会覆盖旧的成功标记，因此新的失败 revision 不会被误报为通过。

`--keep-going` 需要逐 definition 重新收集完整结构化诊断，明确不与成功缓存组合。CI 与发布门禁继续运行不带 `--incremental` 的冷检查。

在 Respo 临时副本上使用 release binary 与 compatibility policy 验证：cold 为 0.32s，连续 warm 为 0.11s；warm 报告 `preprocessing_cached=true`，input 与 dependency index 均为 `warm`。
