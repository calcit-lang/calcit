# Option / Result 谓词统一为方法写法

Option 和 Result 已有 `.some?`、`.none?`、`.ok?`、`.err?` 接收者方法，但旧代码仍混用 `option:*`、`result:*` 内部 helper。沿用现有的 `core-option-method-v1` 与 `core-result-method-v1`，把四个谓词调用纳入可预览、可应用且幂等的迁移，不增加命令行入口或规则名。

自动改写必须同时证明 helper 解析到 core、接收者静态类型能分派到对应的名义方法，且该方法实现指向同一个 helper。裸 `Dynamic` 不能因谓词返回 `Bool` 而绕过严格类型检查。旧 helper 暂留为 core 实现及迁移窗口，不在此时批量废弃或重命名。

公开文档和 core 查询元数据推荐方法写法；Calcit `:tests` 验证四个谓词的行为，CLI 测试覆盖迁移与动态边界，WASM 样例验证其中两个方法可编译执行。真实消费者中 js-ffi、Respo 和 Editor 仍有旧谓词调用，后续按版本与依赖门禁逐个迁移。
