---
title: "快速开始（新 LLM 必读）"
summary: "Agent/LLM 使用 Calcit CLI 的核心原则和标准工作流程：先搜索再修改，用命令行工具而非直接编辑文件"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "quick start"
  - "quick-start"
  - "llm quick start"
  - "standard workflow"
  - "new llm"
entry_for:
  - "cr query search"
  - "cr tree replace-leaf"
---

# 快速开始（新 LLM 必读）

**硬前置步骤：在执行任何 `cr edit` / `cr tree` 修改前，必须先运行一次 `cr docs agents --full`。**

这不是建议项，而是进入实际修改前的检查项。跳过这一步，往往会直接沿用旧用法假设，尤其容易误判 `cr tree replace -p ''`、imports 输入格式和 watcher 验收边界。

**核心原则：用命令行工具（不要直接编辑文件），用 search 定位（比逐层导航快 10 倍）**

### 标准流程

```bash
# 搜索 → 修改 → 验证
cr query search 'symbol' -f 'ns/def'                      # 1. 定位（输出：[3.2.1] in ...）
cr tree replace 'ns/def' -p '3.2.1' --leaf -e 'new'     # 2. 修改
cr tree show 'ns/def' -p '3.2.1'                        # 3. 验证（可选）
```

### 三种搜索方式

```bash
cr query search 'target' -f 'ns/def'                      # 搜索符号/字符串
cr query search-expr 'fn (x)' -f 'ns/def'                 # 搜索代码结构
cr tree replace-leaf 'ns/def' --pattern 'old' -e 'new' --leaf  # 批量替换叶子节点
```

### 效率对比

| 操作       | 传统方法                | search 方法         | 效率     |
| ---------- | ----------------------- | ------------------- | -------- |
| 定位符号   | 逐层 `tree show` 10+ 步 | `query search` 1 步 | **10倍** |
| 查找表达式 | 手动遍历代码            | `search-expr` 1 步  | **10倍** |
| 批量重命名 | 手动找每处              | 自动列出所有位置    | **5倍**  |
