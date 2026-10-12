# 保留生成方法函数的类型合同

真实下游 calcit-regex 迁移到 0.29.0 的方法写法时，`.unwrap (.find pattern text)` 和先绑定 `.find` 结果再调用 `.unwrap` 均失败。完全不依赖 FFI 的 typed Box/trait 最小用例也复现该问题，关联 #1980、#1553。

名义方法 lowering 已把 trait schema 存入生成函数的 compiled payload，但返回类型推断只查询 Snapshot schema。生成函数没有 Snapshot 定义，因此被当成没有返回类型证据。修复在没有源码定义时使用现有 compiled schema；有源码定义时仍以公开源码合同为准，不修改 Dynamic、类型证明或方法选择规则，也不增加迁移规则。

回归放在现有 `calcit/test-traits.cirru` 的 definition `:tests`：嵌套 prefix 方法和局部绑定后的 postfix 方法。原始 0.29.0 两项都失败；候选修复在 native/JS/WASM 三个后端均为 2/2。通过现有统一运行器接入 CI，不新增验证脚本。

候选修复也使 calcit-regex 原有 attached test 1/1、README 代码块 3/3 通过，原断言与方法迁移不变。现有 core 回放为 native 613/613、JS 612/612、WASM 388/388；保留原排除项。现有 strict-default 成功/失败/兼容/冲突/Option/Result/开放容器测试通过。

Rust 单测首次构建测试依赖时因磁盘不足失败，未执行测试，不能计作通过。仅清理本任务旧工作树中的可重建 target 缓存，保留候选 CLI 至 `/private/tmp/calcit-trait-result-1980/bin/calcit`，随后用关闭 test debug info、关闭 incremental、双 job 的配置重新验证。完整 Rust/Clippy/集成检查、最新 head review 和精确 main CI 未验收前，不合并或发布。

低资源重试的 library suite 为 997 passed、1 ignored、0 failed；完整 cargo test 和 Yarn check-all 随后继续执行。Agent interface 为 64/64，view reading tasks 为 6/6。

Regex 完整原生入口随后暴露独立的泛型绑定问题：原有 `Fn<T>(T,T)->Unit` equality helper 先收到 `Option :none` 时将 T 绑定为 `Option<Never>`，拒绝后面的 `Option<String>`。无依赖最小复现在正式 0.29.0 同样失败，交换两个参数即可通过，关联 #1981。该问题不是本修复引入；保持下游 schema、原断言和输入不变，不用独立泛型、Dynamic 或改断言绕过，因此仍不宣称完整消费者迁移完成。
