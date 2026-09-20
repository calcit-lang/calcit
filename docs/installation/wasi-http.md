# 有界类型化 WASI HTTP

Calcit 的首个 WASI HTTP client 以闭合、完整缓冲的 request/response contract 为边界，
面向简单业务调用，不把标准 `wasi:http` 的 resource、stream、pollable 或 transport
对象暴露到表层语言。

首批 contract 包含：

- `HttpMethod`：闭合的常用 HTTP method；
- `HttpHeader`：String 名称与 Buffer 值，避免把非 UTF-8 header 静默改写为文本；
- `HttpBody`：明确区分 empty、UTF-8 text 与任意 bytes；
- `HttpRequest`：method、URL、headers、body 与必填的 `max-response-bytes`；
- `HttpResponse`：`UInt16` status、headers 与 body；
- `HttpError`：capability denied、unsupported runtime、invalid request、transport failure
  与 response-too-large 等闭合错误。

Component 侧通过一个 async `request` import 使用该 contract。它是 Calcit 与宿主 adapter
之间的稳定接缝，不是对标准 `wasi:http` resource API 的重命名。后续由 component adapter
把这个有界调用桥接到标准 outgoing handler、body stream 与 resource 生命周期；JavaScript
adapter 使用同一组 Calcit 类型和错误语义。

响应必须在读取前声明上限。宿主未授予网络 capability、运行时不支持 HTTP、响应超过上限
或协议失败时，adapter 返回对应的 typed error，不能返回伪造的空 response。首批不包含
streaming body、HTTP server、socket/TLS 细节和主动取消；这些能力不会通过增加顶层命令绕过
当前 contract。

可复制路径与版本组合见 [`examples/wasi-http-client/`](../../examples/wasi-http-client/)：
`calcit wasm --boundary component` + `calcit ffi export --boundary component` + `calcit-bindgen 0.1.3`。
当前稳定宿主 adapter 使用 Wasmtime 47 的 WASI 0.2 `wasi:http/outgoing-handler` 生产传输，
Calcit-facing contract 保持 WASI 0.3 原生 async；portable、直接依赖 WASI 0.3 HTTP host 模块的
adapter 仍被其 experimental tooling 阻塞，尚未提供。

