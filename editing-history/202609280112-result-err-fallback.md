# Result 错误分支的备用值推断与方法迁移

## 决策

`%err error` 和 core `Result :err error` 没有成功 payload。调用 `result:unwrap-or` 时，如果备用值有具体类型，可让它确定成功类型，同时从构造器保留错误类型。普通 `Result<Dynamic,E>` 不代表一定是错误分支，不能据此收窄；`%ok` / `Result :ok` 可能实际携带 Dynamic 成功值，也不得用备用值冒充其类型证据。

命名迁移继续遵守角色划分：公开接收者操作首选 `.unwrap-or`，`result:unwrap-or` 暂作 core helper。新增规则沿用 `calcit fix`，显式选择 `core-result-method-v1`，不增加顶层入口或修改已发布 preset。只有源码引用、目标方法、类型、单次求值与顺序都可证明时自动改写；遮蔽、开放值和未知 macro 保留人工审阅。旧 helper 的删除须等待真实消费者完成迁移、文档与 Agent 查询统一首选写法、至少一个发布窗口和受影响后端回归。

## 验证

Calcit definition `:tests` 验证两种已知错误构造器的运行语义；CLI 测试验证类型查询、预览、revision 防护、应用、幂等及开放/遮蔽/动态成功值的负向边界。WASM fixture 改用 `.unwrap-or`，并运行 JS/native/WASM 与项目门禁。当前另观察到 `%ok Dynamic` 的类型查询可能将泛型结果显示得过于精确；本规则对该构造保守拒绝自动迁移，泛型来源问题应另行最小复现和修复，不由 fix 增加特殊推断表。
