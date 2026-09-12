# 公开 WASM/WASI CLI / Public WASM/WASI CLI

## 中文

- 增加 `calcit wasm <snapshot>` 与 `calcit wasi <snapshot>` 两个公开 preview 子命令，让输出类型和宿主契约可以从命令与 help 直接发现。
- 把原 `cr-wasm` 的 Snapshot 加载、entry 选择、预处理、target validation 与 codegen 移到共享模块；`cr-wasm` 仅保留为内部兼容 wrapper。
- 共享 loader 登记项目 namespace，并保留 Snapshot 自带的 core namespace，使公开命令遵循 `calcit` 默认严格诊断，同时不把 bundled core 的兼容定义误判为用户代码。
- 仓库的 core WASM 脚本和 CI 改用 `calcit wasm`；WASI command 生成与 fail-closed 检查改用 `calcit wasi`。旧 core fixture 暂时显式使用 `--compat-types`，新 WASI fixture 保持严格默认。
- 本机已安装的 `wasm32-wasip1` 用于继续验证 `cr-wasm.wasm` 自举路径；生成的 WASI command 由 Wasmtime 启动。

## English

- Added public preview commands `calcit wasm <snapshot>` and `calcit wasi <snapshot>` so output kinds and host contracts are discoverable through command names and help.
- Moved Snapshot loading, entry selection, preprocessing, target validation, and codegen out of `cr-wasm` into one shared module. `cr-wasm` remains only as an internal compatibility wrapper.
- The shared loader registers project namespaces and preserves source-provided core namespaces. Public commands therefore honor the strict `calcit` default without treating bundled compatibility definitions as user code.
- Migrated core WASM scripts and CI to `calcit wasm`; migrated WASI command emission and fail-closed checks to `calcit wasi`. The legacy core fixture temporarily opts into `--compat-types`, while the new WASI fixture stays strict by default.
- The locally installed `wasm32-wasip1` target continues to validate the `cr-wasm.wasm` bootstrap path, and Wasmtime starts the emitted WASI command.
