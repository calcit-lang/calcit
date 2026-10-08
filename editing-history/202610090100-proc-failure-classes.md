# 内建 Proc 的失败类别（#1558）

问题：参数类型正确时，内建 Proc 是否会失败、以什么方式失败，签名里看不到；native 还有输入能让进程 panic。

决策：

- 类别只有三种：`total`、`result`（nil、Option 或 Result 返回预期失败）、`raises`（可捕获错误，WASM trap），后两类附一句条件。类型不符的参数在所有 Proc 上都是类型错误，不逐个重复。
- 分类放在 `src/calcit/proc_failure.rs` 的穷尽 `match`，而不是给 `ProcTypeSignature` 的 200 多个字面量各加字段：新增 Proc 不分类就无法编译，同时不新建 registry。
- 查询只在可证明为 core Proc 的定义上输出 `:failure`，字段登记在 #1566。
- `raise` / `try` / `Result` 分工写入错误处理文档：可恢复失败用 Result/Option，`raise` 表示不可恢复，`try` 用在宿主与旧接口边界。
- 修复 panic 时向 JS 已有行为对齐：`&number:format` 位数上限 100（`toFixed`），pad 长度上限 536870888（V8 字符串上限），NaN 与负长度不填充；`&list:assoc` 越界报错。

兼容边界：`&list:assoc` 越界此前返回由参数组成的列表，现在报错；`&str:pad-left/right` 对 NaN 长度此前按 0 处理，保持不变。

验证：随机输入测试（固定种子，每个非副作用 Proc 500 次）、四个新增 core `:tests`、`cargo test`、全量 core `:tests` 回放。`range` 的内存耗尽另见 #1852。
