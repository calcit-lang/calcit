# Cursor 与 transaction 使用文档

## 概要

- 在 Agent 指南中补充通过搜索结果设置 cursor、用 `@cursor` 进行连续树形编辑、导航栈与结构化 clipboard 的推荐流程。
- 在结构化编辑参考中完整记录 cursor sidecar、focus/node/full 展示、mutation 后坐标维护与 `--cursor-after` 回显控制。
- 记录 Cirru EDN transaction 的主要输入形式、quoted code、dry-run、revision 检查与 JSON 兼容边界。
- 将新增命令加入 Agent 能力地图和文档 frontmatter，便于结构化文档查询定位。

## 一致性

- 文档示例沿用既有 `cr` 子命令风格，不引入 Cargo 风格命令。
- 所有路径仍指向 Snapshot 中的 Cirru tree，展示用 `CURSOR` 不改变真实源码与 path。

