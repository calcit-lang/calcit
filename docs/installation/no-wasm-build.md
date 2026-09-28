# 低资源机器上的 Calcit 安装

从首次包含该资产的版本起，Ubuntu 22.04 x86_64 无需本机编译：从 Calcit 的对应版本 GitHub Release 下载 `calcit-ubuntu22.04-x86_64-no-wasm`，核对同页 `calcit-release-manifest.json` 中的 SHA-256，再执行：

```bash
sha256sum calcit-ubuntu22.04-x86_64-no-wasm
chmod +x calcit-ubuntu22.04-x86_64-no-wasm
./calcit-ubuntu22.04-x86_64-no-wasm -v
```

先将第一行输出的哈希与 manifest 中同名资产的 `sha256` 对照，一致后再执行后两行。

该资产在固定的 `ubuntu-22.04` x86_64 runner 上构建和运行验收，面向同版本或更新的常见 glibc Linux 环境；不同 CPU 架构、较旧 glibc、额外 native FFI 动态库仍需分别确认。它不是静态链接或通用 Linux 二进制。Release 中的常规 `calcit` 资产保留 WASM/WASI 能力。

无 WASM 版本仍支持 Calcit native 解释执行、JS 编译、类型检查、查询、结构化编辑与文档命令。`calcit wasm` / `calcit wasi` 会明确报错，不会静默降级或生成不完整产物。项目仍可保存和检查与 WASM 有关的语法；本二进制不负责输出 WASM/WASI。

若机器资源允许，也可从源码选择相同功能集：

```bash
cargo build --locked --release --no-default-features --bin calcit
```

`cargo install calcit --bin calcit --no-default-features --locked` 也可用，但仍需要本机 Rust 编译资源；低资源机器优先下载已构建资产。需要 WASM/WASI 时使用常规 Release 资产，或按默认 feature 构建；`--no-default-features --features wasm` 可显式重新启用。
