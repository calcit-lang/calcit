# WASI HTTP 起步应用

这个示例把 Calcit 的 buffered typed HTTP Component 边界整理成一条可复制的应用路径。网络默认拒绝；宿主必须显式授予一个精确的 `scheme://authority`。请求、响应与失败使用闭合 Struct、Enum 和 Result，不把 socket、TLS、stream 或 Wasmtime resource 暴露给 Calcit 代码。

它与 `calcit wasi` 的 Preview 1 command 路径不同：HTTP 使用 `calcit wasm --boundary component` 生成 Canonical ABI core module，再由 `calcit-bindgen` 打包。裸 `program.wasm` 不是 runnable Component。

## 前置版本

- Calcit 0.15.6 或当前仓库构建的 `target/debug/calcit`
- `calcit-bindgen 0.1.2`
- Rust toolchain；host 固定使用 Wasmtime 47.0.4

安装已发布的生成器：

```bash
cargo install calcit-bindgen --version 0.1.2 --locked
```

## 构建

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
```

Component contract 默认使用 Cirru EDN；只有 JSON-only consumer 才显式改用 `--format json`。

## 运行

先启动一个本机 HTTP 服务：

```bash
python3 -m http.server 8123 --bind 127.0.0.1
```

另一个终端执行：

```bash
cargo run --manifest-path examples/wasi-http-client/host/Cargo.toml -- \
  http://127.0.0.1:8123 \
  http://127.0.0.1:8123/README.md \
  65536
```

第一个参数是宿主授予的唯一 origin，第二个参数是请求 URL，第三个参数是本次响应体上限。把 URL 改成其他 origin 会得到 typed `capability-denied`；把上限调小会得到 `response-too-large`。redirect 不会自动跟随。

## Calcit 测试

业务层数据形状保存在 definition 的 `:tests` 中：

```bash
calcit examples/wasi-http-client/calcit.cirru test \
  --tag wasm --require-match
```

生成目录和 interface 文件都是可重建产物，不应提交。
