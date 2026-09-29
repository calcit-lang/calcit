# FsPath 写入方法的效果命名

## 决策

按 #1457 的有限规则，应用层 `FsPath` 写入采用 `.write-text!`，因为它明确改变外部文件状态。旧 `.write-text` 暂留兼容，两者在 `FsPathOpsImpl` 中都指向现有 `fs-path:write-text`，不另建一个实现或新的命令入口。

## 语义边界

新名字仍是 `(FsPath,String) -> Result<Unit,String>`，不改为抛错，也不隐式创建路径权限。Native、生成 JS、WASI Preview 1 与 WASI 0.3 Component 继续使用原有文件效果实现；core WASM 仍明确不支持。js-ffi 的 `write-text!` 保持它自己的 Unit/throw/async 失败模型，不因为拼写相近而合并签名。旧方法只有消费者迁移与发布版验证后才考虑退场。

## 验证

定义内 Calcit `:tests` 证明新方法保留 Result 类型与错误分支。WASI command 的两个现有业务入口改用新方法，沿用脚本检查 native/JS/Preview 1/Component 的真实写入、拒绝路径和错误状态。编译器对新方法保留 WASI 文件效果识别，并在 Rust 低层测试覆盖该目标选择边界。
