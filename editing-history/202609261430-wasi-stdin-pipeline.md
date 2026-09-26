# WASI stdin 管道（#1309）

## 目标与边界

复用 `examples/wasi-command` 的 `process-manifest`，完成 stdin Cirru EDN → typed transform → stdout。Native、Node JS、真实 WASI Component 使用相同业务逻辑；浏览器明确不支持 stdin。公共返回值沿用 `Result<String, String>`，不增加公开 stream/Task 框架，不把本任务扩展成通用异步支持。

## 内部 reader

文件 descriptor 与 `wasi:cli/stdin@0.3.1` 的读取都使用字节 stream 和 completion future。共享有界读取实现，只有来源参数及错误载荷释放不同：filesystem 的 `error-code::other` 可能持有字符串，CLI 的 error-code 是无载荷 enum，不能套用文件错误的内存布局。

最多接收 4 MiB，多保留一个字节用于区分“恰好达到上限并 EOF”和“超限”。正常 EOF 后先观察 completion，再释放 readable stream；提前拒绝时先取消 stream，且不能用取消后的成功结果覆盖原来的失败。这个内部函数只返回候选字节，严格 UTF-8 校验属于后续 text lowering，不能把原始字节测试当成完整文本契约。

## 已有证据

- 2026-09-26 核对上游最新正式 Wasmtime 为 49.0.1；本机使用已核对官方 digest 的隔离安装。没有把 CLI 版本当作 Rust 嵌入 host 或 WIT 版本。
- `WASMTIME_CLI=.../wasmtime cargo test --lib wasi_03_bounded_ -- --nocapture`：两个宿主测试通过。
- stdin 真实管道覆盖空输入、Unicode 精确字节、多块输入、4 MiB、4 MiB + 1；以目录 descriptor 作为 stdin 触发 Unix 宿主读取失败，确认不能冒充成功 EOF。
- 原有文件 reader 的正常、空、多块、上限、超限、缺失文件回归保持通过。
- Rust 测试只验证 Canonical ABI/宿主资源边界；业务语义仍应复用 Calcit definition `:tests`。

## 公开接口与实际管道

采用零参数 `read-stdin-text`，沿用 `get-args` 等进程级入口的风格，不为一次性 stdin 读取新增 nominal wrapper 或公开 stream 类型。业务数据仍由 `process-manifest` 的普通 Result 方法处理。返回 `Result<String, String>`；固定 4 MiB 上限，超限、非法 UTF-8 与读取失败返回错误。stdin 已消费的输入不保证可重试；不关闭 native/Node 的 fd 0。Node 按已有文件边界使用显式字节注入，浏览器明确 unsupported。

实际测试发现 native CLI 的计时信息污染 stdout，因此将普通程序一次执行和 watch reload 的计时/返回值改到 stderr，同步已有计时脚本。`eval` 明确查询表达式值，保留原有结果输出。不能要求业务管道用 grep 删除 CLI 的杂项输出。

WASM EDN 解码器的既有 64 KiB 限制独立于 reader 的 4 MiB 限制。本任务没有调整解码器的资源边界。文档明确该差异，测试分别验证普通业务输入和原始 reader 的 4 MiB 契约；不以文本读取成功宣称业务解码成功。

`scripts/test-wasi-stdin.mjs` 已通过 42 个 native/Node/真实 Component 输入案例，覆盖非法/截断/surrogate UTF-8、跨块字符、BOM 保留、空输入、边界大小及业务错误。三份 `process-manifest` definition tests 使用相同 AST 在三后端执行成功，宿主脚本没有重新实现业务转换。三个目标均验证真实宿主读取失败及成功后的重复 EOF，JS 注入缺失/类型错误/异常及 browser unsupported 另有宿主边界断言。Preview 1 必须在编译阶段拒绝 stdin 并提示 `--boundary component`，不能输出运行时 trap 冒充可执行 artifact。

CI 的 Component CLI 从 49.0.0 更新到正式 49.0.1，Linux 资产 SHA-256 从官方 release 元数据核对。没有把嵌入 host crate 或 WIT 版本号冒充同一次升级。

## 待交付

本地 `cargo test`、`cargo clippy --all-targets -- -D warnings`、`yarn check-all` 以及 Respo 严格检查与 42 项 definition tests 已通过。Rust HTTP 测试需要本机监听端口权限，申请后执行原测试通过，未删减案例。合并仍取决于最新 HEAD 的 review/CI；合并后核对精确 main workflow，不能把本地验证等同于已发布。
