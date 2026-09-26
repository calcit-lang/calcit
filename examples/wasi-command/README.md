# WASI command 文本处理示例

## stdin Cirru EDN 管道

`app.main/manifest-stdin-main!` 复用下文文件入口的 `process-manifest`，没有第二套业务转换器。`read-stdin-text` 同步读取至 EOF，严格验证 UTF-8，最多接收 **4 MiB 原始字节**，返回 `Result<String, String>`。超限、非法 UTF-8 或宿主读取失败返回 `:err`；已消费的数据不保证可重试。空 stdin 是合法的空文本，但不是合法的 Manifest，所以业务入口返回 65。读取失败返回 66；错误写入 stderr，stdout 只输出成功的 Cirru EDN。

```bash
cargo build --bin calcit
./target/debug/calcit --init-fn app.main/manifest-stdin-main! \
  examples/wasi-command/calcit.cirru < examples/wasi-command/manifest-input.cirru

./target/debug/calcit --init-fn app.main/manifest-stdin-main! \
  wasi examples/wasi-command/calcit.cirru --boundary component \
  --emit-path target/manifest-stdin
wasmtime run -S p3 -W component-model-async-stackful=y \
  -W component-model-more-async-builtins=y target/manifest-stdin/program.wasm \
  < examples/wasi-command/manifest-input.cirru
```

Component 验证版本为 Wasmtime 49.0.1 / WIT 0.3.1，不需要授予文件目录权限。stdin 只接入 Component，不新增 Preview 1 实现；默认边界切换由 #1269 单独推进。

注意文本读取与业务解码是两层契约：WASM 现有 Cirru EDN 解码器仍有 **64 KiB** 输入限制，因此这个跨目标 Manifest 示例只承诺该范围内的业务输入；超过时 Component 返回业务错误 65（`E_WASM_EDN_INPUT_LIMIT`），不是 reader 的读取错误 66。4 MiB 是原始文本 reader 的上限，不代表 EDN 解码器已扩容。

Node 使用与已有文件能力一致的显式 host injection：`read_stdin(maximumBytes)` 返回至多指定数量的 `Uint8Array` 字节，不自行解码或关闭 fd 0。runtime 检查返回类型、上限与 UTF-8；不能使用无界 `readFileSync(0)` 或提前截断到上限并伪装 EOF。可运行 host 见本目录 `run-stdin.mjs`：

```bash
yarn compile
./target/debug/calcit --init-fn app.main/manifest-stdin-main! \
  --emit-path target/manifest-stdin-js examples/wasi-command/calcit.cirru js
node examples/wasi-command/run-stdin.mjs target/manifest-stdin-js/app.main.mjs \
  < examples/wasi-command/manifest-input.cirru
```

browser 没有 stdin，明确返回 unsupported，不使用 localStorage 模拟。这个功能没有公开 stream/Task API，也不是交互式逐行 reader；连接终端时会等待 EOF。native CLI 的运行耗时及返回值现在写到 stderr，避免污染业务管道；依赖旧 stdout 计时文本的脚本应改读 stderr。

真实跨目标回归入口：

```bash
WASMTIME_CLI=/path/to/wasmtime-49.0.1 node scripts/test-wasi-stdin.mjs
```

该脚本复用 definition `:tests`，并检查空/Unicode/跨块 UTF-8/非法编码/上限/超限输入。宿主 Canonical ABI 与资源错误由 Rust 的 `wasi_03_bounded_` 测试补充，不在 Rust 重写业务转换断言。

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

独立的 `method-eval-main!` 用普通 `Result.map` 验证有副作用的 receiver 与回调实参：native、Node JS 和真实 Component 都按 `receiver`、`argument`、`api!` 的顺序各输出一次。它不改变上面的文件业务输出；两个回归脚本会一同执行这个检查。

```bash
cargo build --bin calcit
bash scripts/test-wasi-manifest-business.sh
WASMTIME_CLI=/path/to/wasmtime-49 bash scripts/test-wasi-manifest-component.sh
```

第一项脚本执行 definition `:tests`、native/Node JS/Preview 1 的正常与失败路径和精确输出比较；第二项脚本复用同一输入/输出，在真实 WASI 0.3 Component 中验证成功、非法业务数据、缺失输入、写入失败和未授予 preopen 的拒绝。
