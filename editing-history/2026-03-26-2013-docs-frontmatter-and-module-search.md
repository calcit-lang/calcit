# Docs frontmatter 与模块检索整理

## 变更概述

- 为 `cr docs search/read/read-lines` 增加 `scope/module` 维度，支持核心文档、模块文档和跨域检索。
- 为 core docs 建立统一的 frontmatter 规范，并补充 `title/scope/kind/category/aliases/entry_for` 元数据。
- 将 `docs` 检索逻辑从纯文件名匹配升级为结合 frontmatter、路径、标题与条目词的综合解析与排序。
- 给文档系统补充规范页与验证页，方便后续扩展 `cr docs` 时保持行为稳定。
- 调整 Cirru 校验错误提示，让“其实是字符串”的场景给出更直接的修复建议。

## 关键实现

- `src/cli_args.rs`
  - 为 `docs search`、`docs read`、`docs read-lines` 增加 `--scope`、`--module` 参数。

- `src/bin/cli_handlers/command_echo.rs`
  - 同步回显新增 docs 参数，确保工具模式下能看到完整检索上下文。

- `src/bin/cli_handlers/docs.rs`
  - 引入 `GuideDocFrontmatter`、`GuideDocScope`、`DocsSearchScope`。
  - 支持解析 Markdown frontmatter，并在加载时校验 `category`。
  - 支持从 core docs 与 `~/.config/calcit/modules/<module>/docs/` 同时加载文档。
  - 统一 `search/read/read-lines` 的查询解析逻辑，允许通过文件名、路径片段、标题、`aliases`、`entry_for` 命中目标。
  - 增加更偏向 guide/reference 页的排序策略，减少 spec/索引页抢占结果。
  - 抽出 `match_score`、`accumulate_match_score`、`parse_guide_doc` 等辅助函数，压缩主文件重复逻辑。

- `src/bin/cli_handlers/docs_tests.rs`
  - 将 docs 相关测试从 `docs.rs` 拆出，单独维护解析、排序、模块加载与校验场景。

- `docs/docs-indexing.md`
  - 固化 frontmatter 字段、分类注册表、scope 布局与 authoring 约束。

- `docs/docs-validation.md`
  - 记录 `cr docs` 的可执行回归命令，覆盖 `search/read/read-lines` 与 module scope。

- `src/bin/cli_handlers/cirru_validator.rs`
  - 当 token 更像“应写成字符串的文本”时，提示使用 `|text` 或 `"|text with spaces"`，减少样式值与括号文本误写的排查成本。

## 文档整理原则

- `category` 只允许稳定注册值，避免搜索元数据失控膨胀。
- `aliases` 主要承接用户会直接输入的别名或术语，`entry_for` 主要承接命令/API/任务入口。
- `Agents.md` 与普通 docs 页复用同一套 frontmatter 机制，但渲染时默认隐藏元数据正文。
- 模块领域知识保留在模块 docs 中，Calcit core 只提供通用索引与加载能力，不内置 Respo 特殊语义。

## 验证

- `cargo fmt`
- `cargo test docs --bin cr`
- 手动验证：
  - `cargo run --bin cr -- docs search target-replace`
  - `cargo run --bin cr -- docs search 'cr eval'`
  - `cargo run --bin cr -- docs read polymorphism.md`

## 后续经验

- docs 元数据一旦进入 CLI 行为，就应该配套规范页与验证页，否则后续加字段很容易出现“文档能写、检索却不稳定”的分叉。
- 模块文档检索要尽量和 core docs 走同一套 resolver，这样使用者不需要记两套命令心智模型。
