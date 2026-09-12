# Compiler-guided source fix

## 背景

#998 要求把类型与预处理阶段已经得到的确定性证据转化为可审阅、可原子应用的 Snapshot source migration，供人类和
AI Agent 共用。首个切片选择既有 `W_REMOVED_DATA_API`，避免为 migration 再造一套扫描器或名称统计规则。

## 实现

- 新增 `calcit fix`：默认 preview，支持 `--format json`、`--ns`、`--def`、`--rule`、`--expect-revision` 与 `--apply`。
- 将 removed data API 的诊断说明与一对一 replacement 集中到同一份 compiler mapping；`tuple?` 保持只读语义选择。
- suggestion 输出 stable rule ID、diagnostic code、surface definition/path、subtree fingerprint、quoted AST replacement 与 applicability。
- 应用流程复用现有 `tree replace` transaction，每个节点带 `--expect` guard；在同目录 staged Snapshot 上重新加载并只预处理
  选定 scope，全部通过后才原子替换源文件。
- 实际写入默认要求干净 Git worktree；非 Git 或已有修改必须分别显式授权 `--allow-no-vcs` / `--allow-dirty`，但不能跳过
  revision、fingerprint、preprocess 或 atomic write。
- 新增中文使用文档和 Agent 操作约束；Agent interface smoke 验证 stdout 只有一个 JSON envelope。
- migration fixture 在 definition `:tests` 中验证替换后的 Calcit 行为；staged validation 同时阻断本规则之外的新 warning。

## 验证

- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`：820 library、335 CLI、23 WASM；fix CLI integration tests 后续扩展为 4 项
- `cargo test --test fix_cli -- --nocapture`
- `node scripts/check-agent-interface.mjs`：32/32
- `bash scripts/check-docs-md.sh`：70 files、331 blocks
- 手动在隔离 Snapshot 上完成 preview、apply、重复 preview，确认第二次 suggestions 为空；另验证 `tuple?` 返回
  `requires-review` 且 `replacement: null`。
- 在维护中的 Respo JS 项目隔离副本内注入同一 legacy call，完成真实 module/Snapshot loader 下的 scoped preview、带 revision
  apply 与幂等复跑；原 Respo 仓库保持只读。

`yarn check-agent-interface` 因该 worktree 未安装 `node_modules` 无法由 Yarn 启动；直接执行其底层 Node 脚本通过，未下载或
改动依赖锁文件。
