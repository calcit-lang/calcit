# Published WASI business smoke / 发布版 WASI 业务 smoke

## 中文

- 使用 crates.io 正式发布的 Calcit 0.18.0 与 calcit-bindgen 0.1.7，从干净 checkout 构建 typed HTTP Component 和生成的 Wasmtime host。
- 业务输入与结果默认使用 Cirru EDN 文件，并只通过显式 preopen 授权；不增加 Calcit 顶层命令或 JSON 默认路径。
- 在 Calcit definition 的 `:tests` 验证请求、响应与错误结构，并在宿主侧覆盖成功、非法输入、网络拒绝、输出权限拒绝、transport 与响应超限的稳定退出码。
- 将这条发布版闭环加入独立 CI job，并记录工具版本、耗时、Component 大小和业务结果；宿主 stdout 保持机器可读但不灌入正常 CI 日志。

## English

- Build the typed HTTP Component and generated Wasmtime host from a clean checkout with the crates.io releases Calcit 0.18.0 and calcit-bindgen 0.1.7.
- Keep Cirru EDN files as the default business input and result format, authorized only through explicit preopens, without adding a Calcit top-level command or JSON-default path.
- Validate request, response, and error structures in Calcit definition `:tests`, then cover stable host exits for success, invalid input, network denial, output denial, transport failure, and response limits.
- Add the published-version loop as a dedicated CI job and report tool versions, elapsed time, Component size, and outcomes while keeping normal host stdout out of routine CI logs.
