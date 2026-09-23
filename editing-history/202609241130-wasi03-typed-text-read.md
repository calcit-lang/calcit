# WASI 0.3 类型化文本读取

沿 `#1267` 与现有单分支 Draft PR，完成 `FsPath.read-text` 的 WASI 0.3 Component lowering。Calcit 表层仍使用 `FsPath -> Result<String,String>`；内部通过 preopen 选择、异步 `open-at`、`read-via-stream` 和 future 完成读取，不暴露 stream、future 或 waitable-set 类型。

读取缓冲最多接纳 4 MiB，额外保留 1 字节用于判定超限。stream 分块读取、pending waitable 事件、EOF 和 future 完成均在内部处理；在错误、超限与正常路径释放 stream、future、descriptor、waitable set 和临时缓冲。成功后先校验 UTF-8，再构造 Calcit String；失败映射为已有 `Result.err` 宿主错误。WASI guest 路径按 `workspace/...` 匹配显式授予的 `/workspace` preopen，未授权路径和不存在的文件不会绕过 preopen。

文件 imports 与 helper 仅在入口依赖图可达文件效果时装配；只有声明了文件定义、但入口不可达的纯计算 command 不携带文件权限，相关回归也覆盖此边界。验证包含 Calcit `:tests` 中的 native 成功/缺失路径，以及真实 Wasmtime 49 Component 中公开 `FsPath.read-text` 的中文内容读取、无效 UTF-8、超过 4 MiB 和缺失文件。低层真实 host 测试另覆盖空文件、多块读取、恰好 4 MiB 边界及超限；CI 固定执行。这一切片仅启用文本读取，`.write-text`、目录效果和其他 WASI 宿主能力仍明确拒绝，不能据此关闭 `#1267` 或把 PR 转为 Ready。
