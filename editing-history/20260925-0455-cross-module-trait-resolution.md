# 跨模块结构体 trait 方法解析

关联 [#1354](https://github.com/calcit-lang/calcit/issues/1354)。Timegrass 使用 `ws-edn.calcit 0.0.32` 与 `cumulo-util.calcit 0.0.23` 时，严格预处理将已标注为 `RetryBackoff` 的 `.reset`、`.next` 报为未知方法。`RetryBackoff` 的定义通过 `impl-traits` 附加 `RetryBackoffOpsImpl`；兼容 JS 构建可运行，但严格检查阻断客户端入口。

原因是命名类型的静态解析在依赖定义尚未求值时，从源码穿过 `impl-traits` 只提取 `defstruct`，因此得到的结构体不含附加实现。方法验证随后只看到核心结构体方法。

本次只在 `TypeRef` 的结构体方法查找处优先读取已求值的 nominal 定义及其 impl；不能取得已求值结构体时，保留原有静态解析和诊断。没有把未知方法改成动态调用，也没有改变 `Dynamic`、trait 优先级或 JS FFI 边界。`calcit/test-traits.cirru` 定义带实现的结构体，`calcit/test.cirru` 从另一个 Snapshot 模块调用方法并记录为表层 `:tests`。

验证：同一条跨模块 `:tests` 用修复前的 0.20.0 编译器运行会报 `.next` 未知，用修复后的编译器运行通过。`cargo test --lib` 859 项、`cargo test --bin calcit` 385 项、`cargo clippy -- -D warnings`、`cargo fmt --check`、`yarn compile`、`yarn check-all`、`yarn check-agent-interface` 通过；生成 JS 对 `.next` 直接调用 trait 实现且 Node 运行通过。用本分支的编译器检查 Timegrass draft #101 时，原来的两处 `.reset/.next` 错误消失，严格检查继续暴露后续应用层错误。
