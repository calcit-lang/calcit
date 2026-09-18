# Define the buffered WASI HTTP contract

## 中文

- 在真实 async Component fixture 中加入闭合的 HTTP method、header、body、request、response 与 error 类型。
- 以 `calcit:wasi-http/client` 的单个 async `request` import 作为 Calcit 与 host adapter 的接缝。
- 请求必须携带 `UInt64` response 上限；响应超限保持 typed error，不伪造空响应。
- 增加 Calcit definition `:tests`、Interface IR contract 检查与 core-module import smoke。
- 标准 `wasi:http` 的 resource/stream lifecycle 留给 bindgen adapter，不进入表层类型或新增 CLI。

## English

- Add closed HTTP method, header, body, request, response, and error types to the real async Component fixture.
- Use one async `request` import under `calcit:wasi-http/client` as the seam between Calcit and a host adapter.
- Require an explicit `UInt64` response limit and preserve overflow as a typed error instead of fabricating an empty response.
- Add a Calcit definition `:tests` contract, Interface IR assertions, and a core-module import smoke.
- Keep standard `wasi:http` resource and stream lifecycle in the bindgen adapter without adding surface types or CLI commands.
