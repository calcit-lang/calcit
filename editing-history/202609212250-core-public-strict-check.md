# Core 公开源码定义进入严格检查

## 背景

移除 core quality 数量预算后，仓库 CI 仍缺少对未被入口引用的 core 定义的严格检查。core Snapshot 本身未声明 target，不能直接用 `--check-only` 或 `check-public` 替代；现有带 WASM target 的测试 Snapshot 可加载当前构建嵌入的 core。

## 发现

使用该 entry 对 `calcit.core`、`calcit.test` 和 `calcit.internal` 运行 `check-public` 时，619 个定义均被枚举。369 个源码定义通过；其余 250 个诊断全部来自 `&runtime-implementation` 占位符：246 条未知符号告警和 4 条 macro syntax 错误。这些占位符没有 Calcit 函数体，不能把它们的预处理结果作为类型正确性判断。

## 调整

- `check-public` 只对内置 `calcit.core` 中带 `:builtin` 标签、且代码精确匹配运行时占位符的定义标记为 `intrinsic`；其他定义仍走相同的严格预处理。摘要分开报告源码通过数与 intrinsic 数，完整性要求覆盖全部已选定义。
- PR 和发布 CI 检查三个 core namespace 的所有源码定义。Agent CLI 协议回归验证结构化输出的完整性与成功状态，不绑定定义数量作为预算。
- 文档明确说明占位符的 schema 仅在加载时校验，实际实现契约仍需 builtin/后端测试验证；该 WASM-target 检查不冒充全部目标的语义覆盖。

## 验证

`yarn check-all` 通过（含 304 个 core definition `:tests`）；`cargo test -q`、`cargo clippy -- -D warnings`、`cargo fmt --check`、`yarn check-agent-interface`、core `check-public` 命令和 Markdown 文档校验（340 个片段）均通过。已有失败路径测试会读取错误摘要中的 `2 passed`，因此保留该文字契约；JSON 的总体通过数与源码/intrinsic 分项仍分别给出。#1238 仍需继续审查 analyzer-only 规则与真实调用方。
