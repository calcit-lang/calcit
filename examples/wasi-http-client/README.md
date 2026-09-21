# WASI HTTP 起步应用

这个示例把 Calcit 的 buffered typed HTTP Component 边界整理成一条可复制的应用路径。网络默认拒绝；宿主必须在 Cirru EDN capability 文件中显式授予精确的 `scheme://authority`。请求、响应与失败使用闭合 Struct、Enum 和 Result，不把 socket、TLS、stream 或 Wasmtime resource 暴露给 Calcit 代码。

它与 `calcit wasi` 的 Preview 1 command 路径不同：HTTP 使用 `calcit wasm --boundary component` 生成 Canonical ABI core module，再由 `calcit-bindgen` 打包 runnable Component 和可直接运行的 Wasmtime host。示例不再维护手写 Rust ABI 层，也不需要源码 path dependency 或新的顶层命令。

当前稳定路径使用 Wasmtime 47 的 WASI 0.2 `wasi:http/outgoing-handler` 生产传输，Calcit-facing Component contract 保持 WASI 0.3 原生 async。portable、直接的 WASI 0.3 HTTP host adapter 仍受 experimental tooling 限制；以后可替换生成器内部 adapter，不改变 Calcit contract、配置格式或这里的命令。

## 前置版本

- Calcit 0.18.0 或当前仓库构建的 `target/debug/calcit`
- `calcit-bindgen 0.1.7`
- Rust toolchain；生成的 host 固定使用已验收的 Wasmtime 版本

安装已发布的生成器：

```bash
cargo install calcit-bindgen --version 0.1.7 --locked
```

## 构建并检查

在 Calcit 仓库根目录执行：

```bash
calcit examples/wasi-http-client/calcit.cirru \
  wasm --boundary component \
  --emit-path examples/wasi-http-client/host/generated-core

calcit examples/wasi-http-client/calcit.cirru \
  ffi export --boundary component --format edn \
  > examples/wasi-http-client/host/interface.cirru

calcit-bindgen generate \
  examples/wasi-http-client/host/interface.cirru \
  --core-module examples/wasi-http-client/host/generated-core/program.wasm \
  --out examples/wasi-http-client/host/generated-component

calcit-bindgen check \
  examples/wasi-http-client/host/interface.cirru \
  --core-module examples/wasi-http-client/host/generated-core/program.wasm \
  --out examples/wasi-http-client/host/generated-component
```

Component contract 默认使用 Cirru EDN；只有 JSON-only consumer 才显式改用 `--format json`。`check` 用于 CI 检测生成物过期，不产生另一套入口。

运行生成 host 后，Cargo 会在受管目录内创建 `Cargo.lock` 与 `target/`。从 0.1.6 起，`check` 会忽略这两个精确的运行产物，下一次 `generate` 可安全重建同一路径；capability 配置和其他用户文件仍必须放在生成目录之外。

## 配置与运行

复制以默认拒绝为基础的 capability 模板，再按业务需要修改本机文件：

```bash
cp examples/wasi-http-client/host/capabilities.example.cirru \
  examples/wasi-http-client/host/capabilities.local.cirru
cp examples/wasi-http-client/host/input/request.example.cirru \
  examples/wasi-http-client/host/input/request.local.cirru
mkdir -p examples/wasi-http-client/host/output
```

配置中的 `:component` 相对配置文件解析。`:allowed-origins` 是唯一网络授权来源。`:arguments-file`
是 guest path，只能通过 read preopen 读取，其中保存完整参数列表；`:result-file` 只能写入 read-write
preopen。参数仍严格按导出函数的 Component 类型解码，不接受 JSON fallback。模板只允许本机
`http://127.0.0.1:8123`，并同时在请求和 host 层限制最大响应体。

先在仓库根目录启动一个本机 HTTP 服务：

```bash
python3 -m http.server 8123 --bind 127.0.0.1
```

另一个终端执行生成的 host：

```bash
cargo run --manifest-path \
  examples/wasi-http-client/host/generated-component/rust/wasmtime-http-host/Cargo.toml \
  -- examples/wasi-http-client/host/capabilities.local.cirru
```

成功结果以 Cirru EDN 同时输出到 stdout 和 `host/output/result.cirru`，诊断输出到 stderr。把 URL 改成
未授权 origin 会得到 typed `capability-denied`；把上限调小会得到 `response-too-large`。redirect 不会
自动跟随。配置值类型不符时，host 会报告带字段路径的错误，不猜测 Dynamic 或回退到 JSON。

生成 host 的稳定退出码为：成功 `0`、输入或 `invalid-request` `2`、capability denied `3`、transport
`4`、response-too-large `5`、unsupported `6`，其他内部失败 `1`。guest 已执行时的 typed error 仍写入
Cirru EDN 结果；参数文件无法解析等调用前 host 错误只写入 stderr，不创建结果文件。退出码负责统一脚本编排。

## 发布版业务 smoke

仓库脚本只调用公开的 `calcit` 与 `calcit-bindgen` 命令，不使用源码 path dependency：

```bash
bash scripts/test-published-wasi-http-business.sh
```

脚本从干净 checkout 构建同一示例，覆盖成功、输入错误、网络/输出目录未授权、transport 与响应超限，
最后再次运行 manifest `check`，并记录版本、耗时和 Component 大小。若本机已经有
`capabilities.local.cirru`、`request.local.cirru` 或 `host/output/`，脚本会拒绝覆盖。

## Calcit 测试

业务层数据形状保存在 definition 的 `:tests` 中：

```bash
calcit examples/wasi-http-client/calcit.cirru test \
  --tag wasm --require-match
```

生成目录、interface 文件、本机 capability、request 与 output 文件都是可重建或环境相关产物，不应提交。
