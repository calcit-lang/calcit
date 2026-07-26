# Command Echo 主导的工具输出收敛与 tree replace 差异摘要

## 变更概述

- 将 `cr` 的命令回显进一步固定为工具主语境：前置、单行、语义化参数展示。
- 为工具型命令引入更安静的 runtime 输出模式，避免版本、模块目录、模块加载、平台 API 注册等噪音干扰结果读取。
- 大范围清理 `docs/query/tree` 正文里对 `target/path/pattern/entry/deps` 的重复回显，统一交给 command echo 表达。
- 修正 `docs/CalcitAgent.md` 中示意性 Cirru 代码块的检查模式，避免 `check-md` 将结构示例误判为 runnable 程序。
- 单独重构 `tree replace` 输出：从“Preview + From/To”双份重复，改成“Changed node + Containing expression”的单次差异摘要。

## 关键实现

- `src/bin/cr.rs`
  - 在启用 command echo 时同步启用 `calcit::set_quiet_tool_output(true)`。
  - 屏蔽工具模式下的运行环境提示：
    - `calcit version`
    - `module folder`
    - `stack trace disabled`
    - `running entry`

- `src/lib.rs`
  - 新增全局 quiet flag：
    - `set_quiet_tool_output(...)`
    - `quiet_tool_output()`
  - `load_module(...)` 在工具模式下不再打印 `loading: ...`。

- `src/bin/injection/mod.rs`
  - `inject_platform_apis()` 在工具模式下不再打印 `registered platform APIs`。

- `src/bin/cli_handlers/docs.rs`
  - `docs check-md` 不再重复回显 `entry/deps`。
  - 移除只服务旧回显的路径展示 helper。

- `src/bin/cli_handlers/query.rs`
  - 删除 `query def/peek/schema/examples/usages` 中重复的 target 标题。
  - 删除 `query search/search-expr` 中重复的 pattern/filter/entry/start-path 摘要。
  - `query find` 摘要从“重复 symbol”收敛为纯结果计数。

- `src/bin/cli_handlers/tree.rs`
  - 删除 `tree show` 与多类 tree 写操作中的冗余 follow-up 命令提示。
  - 删除成功文案里重复的 `path/target/pattern` 回显。
  - `tree replace` 改为：
    - `✓ Replaced node`
    - `Changed node` 下单次展示 `Before/After`
    - 非 root path 额外展示 `Containing expression`，帮助快速看出修改落点。

- `docs/CalcitAgent.md`
  - 将示意性代码块改成 `cirru.no-check`，避免 `docs check-md` 因不存在的符号或非独立程序片段失败。

## 输出策略结论

- 输入参数语境：由 `Command: ...` 统一承担。
- 正文输出：只保留结果、差异、结构上下文、或真正新增的信息。
- 不再在正文里重复打印 command echo 已经覆盖的 target/path/pattern/entry/deps。

## 验证摘要

- `cargo fmt`
- `cargo run --bin cr -- calcit/test.cirru analyze js-escape 'demo?'`
- `cargo run --bin cr -- calcit/test.cirru analyze js-unescape 'demo_$q_'`
- `cargo run --bin cr -- calcit/test.cirru query ns app.main`
- `cargo run --bin cr -- calcit/test.cirru query find render`
- `cargo run --bin cr -- calcit/test.cirru tree show app.main/test-json --path ''`
- `cargo run --bin cr -- calcit/test.cirru docs search chunk --filename agent-advanced.md`
- `cargo run --bin cr -- demos/calcit.cirru docs check-md docs/CalcitAgent.md`
- `cargo run --bin cr -- /tmp/calcit-cli-demo/calcit.cirru tree replace ...` 多次人工检查 replace 输出形态

## 经验

- command echo 一旦语义化，就应把正文里的“参数再描述一遍”系统性删掉，否则会让工具输出看起来像教程而不是结果。
- tree 类命令相比简单成功提示，更需要“差异摘要 + 所在表达式”，这样既不冗余，也能快速判断改动是否落在预期结构。
