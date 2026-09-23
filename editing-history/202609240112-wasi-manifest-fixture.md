# 有界 WASI 文件业务的先行验收样例

0.20 的第一步先固定一个具名配置变换程序，而不是先实现 WASI 0.3 文件接口再补测试。`Manifest` 在 Cirru EDN 解码时一次性收窄为具名 Struct；之后通过普通方法处理 `Result`，文件读写继续使用现有 `FsPath` 方法。输入无效、文件缺失和输出失败分别保留不同退出码，业务错误不能产生输出。

当前 native、Node JS、Preview 1 共用同一 Snapshot 与输入/输出数据。WASI 0.3 Component 仍明确拒绝文件能力；本次回归把这种拒绝固定为过渡状态，#1267 实现 lowering 后必须把它替换为真实 Component 成功与失败回归。此切片不增加 Calcit 顶层工具入口，不引入 Dynamic/unsafe 或手写 WIT host glue。

样例同时暴露了 JS `match` 的 Unit 分支会落入 fallback 的问题：具名 `:ok` 已匹配并执行 `println`，却继续抛出未匹配异常。修复位置在 indexed match codegen，仅在分支体未自行返回时离开 wrapper；已有值返回分支保持原行为。验证以 definition `:tests` 和 native、JS、Preview 1 真正执行同一配置变换为准。
