# 补全增量严格检查的冷启动与绕过契约

## 背景

CodeRabbit 指出 Agent 指南与 CLI 文档没有完整列出 `--check-only --incremental` 的冷启动及保守绕过条件，容易让调用方把不完整的 closure 证据误认为可复用依据。

## 调整

- 在 Agent 指南中明确 reachable definition/schema、namespace import、entry type-slot、policy、编译器与 core input 变化均要求冷启动。
- 在两处 CLI 说明中同步上述条件。
- 明确 closure 证据缺失、存在 unresolved dependency 或 module load 失败时执行普通检查或 bypass，且不复用、不更新旧成功标记。

## 验证

- 运行 Markdown 格式与相关文档文本检查。
