# WASI HTTP 服务接入 `calcit:db@0.1.0` 数据库能力

`calcit` 与 `calcit-bindgen 0.1.12` 之间确立了受控的 WASI HTTP+DB 服务契约：`calcit wasi --world http` 仅额外放行 `calcit:db/sql@0.1.0` 与 `calcit:db/kv@0.1.0` 的同步 `defwasm-import` 声明，`defwasm-export`（`E_WASI_HTTP_EXPORT`）与非 DB 自定义导入（`E_WASM_CAPABILITY`）及 `command` 专属能力（`E_WASI_HTTP_CAPABILITY`）继续按既有边界拒绝。新增 `E_WASI_HTTP_DB_IMPORT` 专用校验，要求每个导入严格匹配固定符号名、同步调用与 `DbError`/`DbParam`/`DbExecResult`/`DbQueryResult` 的字母序名义结构；存在 DB 导入时改由 `calcit_bindgen::package_wasi_http_service` 打包（SQL-only 组件自动省略 `kv`），无 DB 导入时保持原有 `package_wasi_http_proxy` 字节输出不变。

测试层面遵循 Calcit-first 与职责分离原则：错误状态码映射与纯路由行为写在 `tests/fixtures/wasi-http-db.cirru` 定义附带的 `:unit` `:tests` 中；Wasmtime 宿主的默认拒绝策略、SQLite 事务提交/回滚/泄漏兜底清理、KV 字节往返及重复请求的线性内存峰值与吞吐验证通过 `[dev-dependencies]` 启用 `calcit-bindgen/wasmtime-db` 放在 `tests/wasi_http_db_cli.rs`，确保 `calcit` 主二进制本身不引入 SQLite、tokio 或 Wasmtime 运行时依赖。
