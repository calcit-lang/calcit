## agent-advanced.md 拆分: CLI 命令和语言基础节替换为交叉链接, 开发调试拆出独立文件

### 改动

**`run/agent-advanced.md` 精简 (1598→807 行, ~49%):**

- `## Calcit CLI 命令` 节 (~508行): 替换为独立文件链接列表，保留 `LLM 辅助：动态方法提示` 和 `复杂表达式分段组装策略` 两个独特子节
- `## Calcit 语言基础` 节 (~266行): 替换为 cirru-syntax.md / features/tuples.md / features/static-analysis.md 链接，保留 `其他易错点`
- `## 开发调试` 节 (~99行): 拆出为独立 `run/debugging.md`

**新增文件:**
- `run/debugging.md` — watcher 监听模式、增量更新、编译检查

### 删除的重复内容
CLI 命令和语言基础的子节内容在 `run/query.md`、`run/edit-tree.md`、`cirru-syntax.md`、`features/static-analysis.md` 等文件中已有完整覆盖。
