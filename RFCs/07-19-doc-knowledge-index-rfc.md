# RFC: 文档知识节点与增量关系索引

状态：Draft  
日期：2026-07-19  
关联：`docs/docs-indexing.md`、`07-06-semantic-tree-navigation-rfc.md`

## 1. 概要

在保持 Markdown 可读、可 Git 管理的前提下，为 `calcit docs` 增加一层结构化的文档知识索引：

```text
Markdown / Calcit snapshot
        ↓
文档节点与关系解析
        ↓
~/.config/calcit/docs-cache/
        ↓
搜索、跳转、LLM context、缺失检查
```

Markdown 和 Calcit 源码仍然是事实来源；缓存只是可以删除并重新生成的派生数据。

## 2. 目标

1. 文档可以表达稳定的知识节点，而不要求一次性拆分所有 Markdown。
2. 通过 `id`、`parent`、`related`、`requires`、`leads_to` 组织树与图关系。
3. 通过 `namespace/definition` 将文档节点关联到真实 Calcit 定义。
4. 使用用户级 JSON 文本缓存，不引入 SQLite 或其他二进制数据库。
5. 文件内容变化时增量失效；解析器或 schema 变化时全量失效。
6. 能发现孤立节点、断链、未文档化定义和缺少必要章节。

## 3. 非目标

- 当前不引入 RDF、SPARQL、Neo4j 或 SurrealDB。
- 当前不强制把每个章节拆成独立文件。
- 当前不把缓存当作用户需要手工维护的知识源。
- 当前不改变既有 `calcit docs search/read` 的默认输出语义。

## 4. 文档元数据

现有字段继续保留：`title`、`summary`、`scope`、`kind`、`category`、`aliases`、`entry_for`。

新增字段采用稳定 ID 和关系字段：

```yaml
id: api/list/nth
code_refs:
  - calcit.core/nth
parent: structure/list
related:
  - api/list/get
requires:
  - concept/indexing
leads_to:
  - example/list-access
```

实现阶段允许使用 `x-calcit` / `extensions` 命名空间保存项目专属字段，避免污染通用 frontmatter。

`id` 是语义身份；`namespace/definition` 只是代码引用，不能互相替代。

## 5. 缓存模型

默认位置：

```text
~/.config/calcit/docs-cache/v1/<project-id>/
```

项目身份至少包含项目根目录，避免不同项目的相对路径发生冲突。缓存还记录解析器/schema 版本和内置 Calcit core snapshot 的定义指纹；任一输入变化都会触发重建。

缓存由两部分组成：

```json
{
  "nodes": [],
  "edges": []
}
```

每个文件还需要记录：

```json
{
  "path": "docs/structures/list.md",
  "content_hash": "...",
  "parser_version": 1,
  "schema_version": 1,
  "node_ids": ["structure/list"]
}
```

内容 hash 不变时可以复用文件级解析结果；parser/schema 版本变化时必须重新解析。

## 6. 关系模型

树关系：

- `parent`
- `contains`
- `leads_to`

语义关系：

- `related`
- `requires`
- `documents`
- `implements`
- `example_of`

节点采用树形入口，关系采用独立边表，避免把循环关系硬塞进嵌套结构。

## 7. 分阶段计划

### Phase A：元数据兼容与增量缓存基础

- 扩展 frontmatter 解析器，保留未知扩展字段的兼容空间；
- 定义节点、边、文件缓存的 JSON schema；
- 增加内容 hash、parser version、schema version；
- 保持现有搜索和读取命令兼容；
- 添加解析和失效测试。

### Phase B：导航查询

已完成第一版面向知识节点的查询：

```bash
calcit docs graph build
calcit docs graph check
calcit docs graph children <node>
calcit docs graph related <node>
calcit docs graph path <from> <to>
```

查询当前从 Markdown 构建 JSON cache，并通过双向 BFS 查找关系路径：

```bash
calcit docs graph path core/features/list core/run/edit-tree
# core/features/list -> core/run/query -> core/run/edit-tree
```

当前已支持通过 `code_refs` 反查关联文档节点；后续再增加源码定义、示例和正文聚合：

```bash
calcit docs graph explain <definition>
```

### Phase C：完整性检查

```bash
calcit docs graph check
calcit docs graph orphans
calcit docs graph missing
```

检查断链、孤立节点、无正文定义、缺少示例和缺少必要章节。

当前已实现：

- `calcit docs graph check`：检查关系边是否指向已知文档节点；
- `calcit docs graph missing [--ns <prefix>] [--limit <n>]`：按 namespace 分批检查带有 snapshot 文档说明、但没有 `code_refs` 的定义；
- `calcit docs graph orphans`：检查没有任何关系的文档节点。

## 8. 验证标准

- 删除缓存后，查询结果与重新构建结果一致；
- 只修改一个 Markdown 文件时，不重新解析其他未变化文件；
- 修改 parser/schema 版本时会全量失效；
- 不带新增元数据的旧 Markdown 行为保持不变；
- 缓存只包含 JSON/文本，不提交到 Git；
- `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test` 通过。

### 当前验证矩阵

Phase A 当前已验证：

| 案例 | 结果 |
| --- | --- |
| 没有 frontmatter 的旧文档 | 保持空知识元数据，不影响旧搜索 |
| 只有 `id` / `parent` | 正确解析树入口 |
| 单值和多值关系混用 | 保持声明顺序并正确合并 |
| `related` / `requires` / `leads_to` 交叉关系 | 可表达树外关系 |
| 旧的 `title` / `aliases` / `entry_for` | 不会误解析为知识关系 |
| 未闭合 frontmatter | 丢弃知识元数据，避免部分解析污染索引 |
| 文件内容变化 | 内容 hash 失效 |
| parser/schema 版本变化 | 缓存失效 |
| `code_refs` 反查 definition | 返回关联文档节点 |
| 不同项目根目录 | 缓存路径隔离 |
| nodes/edges/files JSON 往返 | 保持结构和关系信息 |

当前仍未接入的能力：

- 从 Calcit snapshot 自动生成 `code_refs`（当前已加载内置 core snapshot，并校验引用是否可解析；应用/模块 snapshot 仍未接入）；
- 自动根据 definition 的 `:examples` 检查文档示例覆盖；
- 从 graph 节点聚合正文、schema、examples 的 LLM context。
