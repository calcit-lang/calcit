# 隔离预处理单测的严格类型策略

## 背景

主分支 push workflow `35378408496` 中，`checks_struct_method_arg_types` 偶发读取到并行测试设置的 strict types 全局状态，错误进入严格 method dispatch 检查并报 `E_ORIGINLESS_METHOD_DISPATCH`。PR 检查与本地全量测试此前均可通过，说明失败来自测试间共享状态竞态，而不是 WASI HTTP starter 的运行语义。

## 调整

- 让依赖兼容模式的 method argument 与 invalid struct field 测试进入统一的 preprocess 测试状态锁。
- 在这些测试作用域内显式设置 `strict_types=false`，退出时由 guard 恢复旧值；高并发验证发现的第二个污染点也纳入同一修复。
- 生产构建继续使用进程级 policy 状态；仅在 Rust 单元测试构建中把 strict types、dynamic-method warning 和 project namespaces 三项 preprocess policy 存入线程局部状态。压力循环先后捕获 strict policy 与 project namespace 交叉污染，说明必须统一隔离策略状态，而不能只逐个补锁。
- 增加确定性的双线程回归测试，让一个线程同时开启三项策略，并验证另一线程仍保持自身的 compatibility/default 状态。
- 不修改生产环境的严格类型规则，也不通过重跑 CI 回避非确定性失败。

## 验证

- 高并发重复运行 preprocess 单测，确认兼容与严格策略不会交叉污染。
- 运行完整 Rust 测试、格式检查和 Clippy。
