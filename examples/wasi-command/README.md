# WASI command 文本处理示例

这个示例展示 Calcit 当前可直接用于小型批处理业务的最短路径：读取命令行参数和环境变量，从 Wasmtime 显式预开放的目录读取文本，转换后写回文件，并用稳定的进程状态码报告失败。它使用默认的 WASI Preview 1 路径；WASI 0.3.1 `--boundary component` 已支持参数、环境变量、标准输出/标准错误和退出码，但尚未覆盖文件能力。

它只使用公开的 `calcit wasi` 入口，不需要 `cr-wasm`、JavaScript host import 或自定义 descriptor API。Calcit 程序只看到 guest path；host path 和授权范围由启动 Wasmtime 的命令决定。

## 构建与运行

先准备一个工作目录：

```bash
mkdir -p target/wasi-command-data
printf '%s' 'payload' > target/wasi-command-data/input.txt
```

从仓库源码构建 Calcit 并生成 WASI command module：

```bash
cargo build --bin calcit
./target/debug/calcit wasi examples/wasi-command/calcit.cirru \
  --emit-path target/wasi-command-example
```

把 host 的工作目录授权为 guest 中的 `workspace`，然后传入 guest 输入、输出路径：

```bash
wasmtime run \
  --dir ./target/wasi-command-data::/workspace \
  --env WASI_PREFIX='prefix: ' \
  target/wasi-command-example/program.wasm \
  workspace/input.txt workspace/output.txt

cat target/wasi-command-data/output.txt
# prefix: payload
```

`WASI_PREFIX` 未设置时默认为空字符串。程序不会接受 host 的绝对路径，也不能越过预开放目录访问文件。

## 失败约定

- 参数不足：状态码 `64`。
- 输入读取失败：状态码 `66`，错误写入 stderr。
- 输出写入失败：状态码 `73`，错误写入 stderr。

例如把输入路径改成 `workspace/missing.txt`，进程会返回 `66`，不会生成空输出伪装成功。

## 测试

纯文本转换语义保存在 definition 的 `:tests` 中，方便直接从 Calcit 代码 review：

```bash
./target/debug/calcit examples/wasi-command/calcit.cirru test \
  --tag wasi --require-match
```

仓库的 `scripts/test-wasi-preprocess.sh` 还会使用真实 Wasmtime 验证文件写入和缺失输入的退出状态。

## Cirru EDN 配置变换（0.20 验收样例）

同一 Snapshot 还提供 `app.main/manifest-main!`：从预开放目录读取 `Manifest`，用 `try-parse-cirru-edn-as` 解码为具名 Struct，验证名称与版本，再把名称加上 `prod-` 前缀并写回 Cirru EDN。`process-manifest` 中的 `.and-then` / `.map` 和文件操作中的 `.read-text` / `.write-text` 都是普通方法调用；非法业务数据返回 `Result` 错误，不会先创建输出文件。输入、错误输入和期望输出分别在 `manifest-input.cirru`、`manifest-invalid.cirru`、`manifest-output.cirru`。

目前该文件入口已在 native、Node JS 和真实 Preview 1 Wasmtime 执行；它是后续 WASI 0.3 文件 lowering 的共享验收程序，**尚不能作为 0.3 Component 运行**。显式 `--boundary component --check-only` 现在应报告 `E_WASI_COMMAND_CAPABILITY`，且不产出 artifact。不要把 Preview 1 的成功当作 0.3 的完成证明。

```bash
cargo build --bin calcit
bash scripts/test-wasi-manifest-business.sh
```

该脚本执行 definition `:tests`、跨后端正常与失败路径、精确输出比较，并验证 0.3 当前的明确拒绝。后续 #1267 接入 0.3 文件能力时，同一输入/输出与失败约定应直接用于真实 Component 回归，不再另造一套业务逻辑。
