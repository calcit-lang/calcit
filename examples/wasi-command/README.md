# WASI command 文本处理示例

这个示例展示 Calcit 当前可直接用于小型批处理业务的最短路径：读取命令行参数和环境变量，从 Wasmtime 显式预开放的目录读取文本，转换后写回文件，并用稳定的进程状态码报告失败。默认仍生成 WASI Preview 1 模块；显式传 `--boundary component` 则生成支持上述文本读写的 WASI 0.3.1 Component。两条路径都要求 host 授予预开放目录；Component 文本读写各限 4 MiB。

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

该文件入口已在 native、Node JS、真实 Preview 1 和 Wasmtime 49 的 WASI 0.3 Component 执行，四条路径使用同一份业务逻辑与期望输出。Component 必须显式使用 `--boundary component`，运行时启用 `-S p3 -W component-model-async-stackful=y -W component-model-more-async-builtins=y`，并用 `--dir HOST::/workspace` 授予预开放目录。`--check-only` 验证能力但不产出 artifact；缺失 preopen、非法输入和写入失败分别返回稳定的退出码。文件写入使用 create + truncate，失败时可能留下截断或部分输出，不承诺原子替换。

类型查询可检查这条业务路径：`query type-at app.main/transform-manifest --path @3` 返回精确的 `Result<Manifest, String>`，`query type-at app.main/process-manifest --path @3` 返回 `Result<String, String>`；后者的 `.and-then` 被静态 lowering，闭包中的 `manifest` 和 `updated` 都保持具名 `Manifest`。Cirru EDN 文本只在 `try-parse-cirru-edn-as` 处解码为具名结构，业务代码不靠 `Dynamic`、`unsafe-coerce` 或 native call 绕过类型。

```bash
cargo build --bin calcit
bash scripts/test-wasi-manifest-business.sh
WASMTIME_CLI=/path/to/wasmtime-49 bash scripts/test-wasi-manifest-component.sh
```

第一项脚本执行 definition `:tests`、native/Node JS/Preview 1 的正常与失败路径和精确输出比较；第二项脚本复用同一输入/输出，在真实 WASI 0.3 Component 中验证成功、非法业务数据、缺失输入、写入失败和未授予 preopen 的拒绝。
