# 实用 WASI HTTP starter

## 背景

0.16.1 已经证明 buffered typed HTTP contract 能在 JavaScript 与 Wasmtime 下访问真实 HTTP 服务，但主要证据仍位于测试 fixture。用户还缺少一条可复制的 Calcit source → Component contract → runnable Component → capability-gated Wasmtime host 链路。

## 本次整理

- 从已验证的 Component contract 提取用户示例，只保留 HTTP Struct/Enum、async import/export 与 definition tests。
- 使用 `calcit-bindgen 0.1.2` 生成 runnable Component 和 Wasmtime HTTP adapter；host 依赖固定 Wasmtime 47.0.4，不使用源码 path dependency。
- Wasmtime host 默认拒绝网络，只接受命令行显式授予的精确 origin，并要求每次请求给出响应体上限。
- 现有 Wasmtime 集成测试改为编译示例 Snapshot，使 starter 与内部 ABI 回归共用同一份 Calcit source。
- 文档明确区分 `calcit wasi` command 与 `calcit wasm --boundary component` HTTP 路径；contract 继续默认使用 Cirru EDN，不增加 CLI 入口。

## 验证重点

- Calcit definition `:tests` 固定请求、响应及 typed error 形状。
- 发布版 `calcit-bindgen 0.1.2` 的 `generate` / `check` 可重建全部 artifact。
- 真实 loopback HTTP 验证成功、capability denied 和 response-too-large；现有跨宿主测试继续覆盖 malformed URL 与 redirect rejection。

