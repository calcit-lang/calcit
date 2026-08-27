# RFC: `calcit analyze program-diff <GIT REF>`

状态：Draft
日期：2026-05-12

---

## 1. 概要

新增命令：

- `calcit analyze program-diff <GIT REF>`

该命令从 Git 中读取指定版本的完整运行时快照文件（默认沿用当前 `calcit` 的 canonical `calcit.cirru` 输入路径；退休的 `compact.cirru` 会被拒绝并提示迁移），确保**历史版本**与**当前工作区版本**都能被正确解析为 Calcit snapshot，然后做**结构化 diff**。

输出目标不是纯文本行 diff，而是面向程序结构的树形 diff：

- 顶层展示 `package/configs/entries/files` 等结构；
- 文件层展示 `ns/defs`；
- definition 层展示 `doc/schema/examples/code`；
- `code/examples/ns.code` 内部继续深入到 Cirru 表达式节点级别；
- 无变化分支允许灰色折叠；
- 变更分支默认展开；
- 增加、删除、修改分别用不同符号和颜色表示。

## 2. 动机

当前针对运行时快照文件的版本对比主要依赖 Git 行 diff，但它有几个明显问题：

1. **对结构不友好**：`calcit.cirru` 是机器生成的 snapshot，行 diff 容易受格式、字段顺序和长表达式影响；
2. **对表达式内部变化不友好**：definition 代码改动往往是局部子树变化，行 diff 很难快速看出 AST 层面的增删改；
3. **对 LLM / CLI 阅读不友好**：调用 `git diff` 后很难一眼区分“只是某个 def 变了”还是“整个 namespace 被搬动了”；
4. **缺少 parse guard**：历史版本文件如果本身损坏，应该先明确报解析失败，而不是直接进入错误 diff。

因此需要一个基于 Calcit 自身 snapshot/Cirru 结构的 diff 命令。

## 3. 命令定义

```/dev/null/rfc.txt#L1-2
calcit analyze program-diff <GIT REF>
```

语义：

- `<GIT REF>` 可以是 `HEAD~1`、tag、branch、commit SHA 等；
- 当前侧使用 `calcit` 输入路径（默认是 canonical `calcit.cirru`；退休的 `compact.cirru` 不参与 diff）；
- 历史侧使用 `git show <ref>:<repo-relative-input-path>` 读取文件内容；
- 两边都必须先经过：
  1. `cirru_edn::parse`
  2. `snapshot::load_snapshot_data`
- 任一侧失败都直接返回错误并停止 diff。

## 4. 输出设计

### 4.1 总体结构

输出分 2 段：

1. Summary
2. Tree Diff

Summary 示例：

```/dev/null/rfc.txt#L1-6
# Program Diff
- ref: HEAD~1
- file: calcit.cirru
- parsed: ok / ok
- changes: ~12 +3 -1
```

### 4.2 树形 diff 约定

参考 `calcit analyze call-graph` 的树形展示风格，继续使用：

- `├──`
- `└──`
- `│`

在此基础上增加状态符号：

- `=` unchanged（灰色）
- `~` modified（黄色）
- `+` added（绿色）
- `-` removed（红色）

### 4.3 折叠策略

- `Unchanged` 子树默认只显示一行概要，并灰色折叠；
- `Modified` / `Added` / `Removed` 子树默认展开；
- root 总是展开；
- 叶子节点直接显示 old/new 摘要。

### 4.4 表达式内部 diff

对于 `code/examples/ns.code` 内部的 `Cirru`：

- 叶子变更显示 old/new；
- list 节点按子节点序列做结构化比对；
- 插入/删除节点要保留索引位置和节点摘要；
- 同位置替换优先显示为 `modified`，而不是简单拆成 delete+add；
- 整棵新增/删除表达式应继续展开其子结构。

## 5. 范围

本次只在 `calcit` 当前仓库内实现，不先抽到 `cirru_parser`：

- 先满足 CLI 工作流；
- diff 数据结构先放在当前项目中，便于快速迭代；
- 后续如果算法稳定，再考虑抽取成 Cirru parser / edn 侧通用模块。

## 6. 实现方案

### 6.1 新模块

新增库模块：

- `src/program_diff.rs`

职责：

1. 读取当前 snapshot 文件；
2. 从 git ref 读取历史 snapshot 文件；
3. 校验两边都能 parse/load；
4. 构建结构化 diff tree；
5. 生成适合终端的彩色树形输出。

### 6.2 CLI 接入

- `src/cli_args.rs`
  - 为 `AnalyzeSubcommand` 增加 `ProgramDiff`；
- `src/bin/cr.rs`
  - 将该命令作为 standalone analyze 命令优先处理；
- `src/bin/cli_handlers/command_echo.rs`
  - 增加 command echo 渲染；
- `src/bin/cli_handlers/mod.rs`
  - 导出 handler。

### 6.3 Git 文件读取

流程：

1. 用 `git rev-parse --show-toplevel` 找 repo root；
2. 将当前 `input` 解析成 repo-relative path；
3. 用 `git show <ref>:<relative-path>` 读取历史内容；
4. 若文件不存在、ref 不存在、或命令失败，则直接返回错误。

### 6.4 Diff 层级

优先按 snapshot 语义层比较：

1. `package`
2. `about`
3. `configs`
4. `entries`
5. `files`
6. `file.ns`
7. `file.defs`
8. `def.doc`
9. `def.schema`
10. `def.examples`
11. `def.code`
12. `Cirru` 子树

这样可以避免直接把整个运行时快照文件当作匿名 EDN 树来比，输出会更稳定、更可读。

### 6.5 Cirru 序列 diff

list 内部子节点对比采用“带替换代价的序列对齐”策略：

- 完全相等 => `match`
- 不相等但对位更合理 => `replace`
- 仅旧侧存在 => `remove`
- 仅新侧存在 => `insert`

这比单纯按索引逐位比较更适合表达式中间插入/删除节点的场景。

## 7. 预期收益

- 更适合 review canonical `calcit.cirru` 的语义变化；
- 更容易理解大型 definition 的局部 AST 改动；
- 对 agent/LLM 更友好，能直接拿到层级化变化；
- 为后续抽象成 Cirru diff 模块提供实现样本。

## 8. 暂不处理

本 RFC 本轮不覆盖：

- 三方 merge；
- patch 应用；
- HTML/TUI 交互界面；
- 抽取到 `cirru_parser` crate；
- 和 `calcit tree show` / `calcit query def` 的统一 chunk/diff UI。

## 9. 后续可能扩展

后续可考虑继续增加：

- `--format json`
- `--no-color`
- `--expand-all`
- `--only <path-prefix>`
- `--git-path <path>`（脱离当前 input）
- 将 Cirru diff 算法下沉到 parser 层
