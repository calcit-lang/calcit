# 2026-08-30 WASM-safe library dependencies / WASM 安全的 library 依赖

- Move CLI-only download, filesystem locking, home-directory, and file-watch
  dependencies behind the non-wasm target boundary.
- Add a CI check that compiles the Calcit library for
  `wasm32-unknown-unknown`, preventing native CLI dependencies from leaking
  back into browser runtimes.
- Keep the local JS regression command aligned with CI by installing
  `scripts/main.mjs` after code generation, so clean workspaces have a launcher.
- Validate the boundary through the real `calcit-lang/wasm-play` embedded
  runtime upgrade.
- 将 CLI 专用的下载、文件锁、home 目录与文件监听依赖移到 non-wasm target
  边界之后。
- CI 新增 `wasm32-unknown-unknown` Calcit library 编译检查，防止 native CLI
  依赖重新泄漏到浏览器 runtime。
- 将本地 JS 回归命令与 CI 对齐，在代码生成后安装 `scripts/main.mjs`，避免干净工作区
  因缺少启动入口失败。
- 通过真实 `calcit-lang/wasm-play` 内嵌 runtime 升级验证该边界。
