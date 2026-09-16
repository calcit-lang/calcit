# WASI command 实用起步项目

- 当前 WASI command 已足够支持参数、环境变量、日志、稳定退出码和预开放文本文件读写，应先把这些能力组合成可复制的业务闭环，再继续追求高级 Component async 完整度。
- 示例只使用 `calcit wasi` 与 Wasmtime，保持公开工具入口收敛；host 负责 preopen 授权，Calcit 程序只传 guest path。
- 用户可观察的纯转换行为放在 Calcit definition `:tests`；shell 回归只验证真实 Wasmtime、文件副作用与退出状态。
- 示例用 `64`、`66`、`73` 分开表示参数、输入和输出失败，避免 host I/O 错误被吞掉或伪装成成功。
