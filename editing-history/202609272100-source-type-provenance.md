# 源码位置上的类型证据补全

## 问题

`get-env` 已有 `Option<String>` 类型，但直接调用经过预处理后可能变成没有源码坐标的内部调用。`query type-at` 因而报告 `unknown`，`core-option-method-v1` 也只能把安全的 `option:unwrap-or (get-env ...)` 标成待人工审阅。这是源码与编译结果的关联缺失，不应增加针对 `get-env` 的特殊推断规则。

## 处理

预处理器增加按需启用的表达式证据追踪，记录原调用位置、正常预处理结果和当时词法作用域中的推断类型。查询和改写在常规定位失败时，仅接受与目标源码位置唯一匹配的证据。已有的解析引用、Option 方法契约和动态类型边界仍决定自动改写的安全性；不新增按函数名猜测返回类型的路径。

## 验证

对两个 WASI Snapshot 的直接 `get-env` 表达式要求 `query type-at` 返回精确的 `Option<String>`。对独立 Snapshot 中的 `option:unwrap-or (get-env ...)` 要求预览、应用和验证均成功，并保留原有开放类型及函数值等需审阅的负例。完整 Rust、Calcit 和文档检查在 PR 中执行。
