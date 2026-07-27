## 文档索引改进: abstract 字段 + --summary 模式 + Hub 标记

### 改动内容

**P0: abstract/summary 字段**
- `GuideDocFrontmatter` 新增 `summary: Option<String>` 字段
- `parse_doc_frontmatter` 支持解析 `summary:` 键值对
- `score_metadata_hit` 加入 summary 评分（exact 200，contains 130）
- 8 个核心文档已添加 summary 描述

**P0: `cr docs search --summary` 模式**
- `DocsSearchCommand` 新增 `--summary` 开关
- 搜索时只显示文档标题 + summary，不输出内容片段
- 无 summary 的文档显示 `(no summary)`

**P0: Hub 标记**
- 搜索结果中 `kind: hub` 的文档显示 `[Hub]` 前缀
- 帮助 LLM 快速识别导航页 vs 详细页

**文档更新**
- `docs-indexing.md` 新增 `summary` 字段规范说明和 `--summary` 用法
- `docs-validation.md` 新增 `--summary` 和 hub 标记的验证用例
