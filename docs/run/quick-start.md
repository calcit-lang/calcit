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
  - "calcit query search"
  - "calcit tree replace-leaf"
id: core/run/quick-start
parent: core/run
---

# 快速开始（新 LLM 必读）

**硬前置步骤：在执行任何 `calcit edit` / `calcit tree` 修改前，必须先运行 `calcit docs agents --contract`。首次使用、contract digest 变化或任务超出紧凑契约范围时再运行 `calcit docs agents --full`。**

这不是建议项，而是进入实际修改前的检查项。跳过这一步，往往会直接沿用旧用法假设，尤其容易误判 `calcit tree replace --path ''`、imports 输入格式和 watcher 验收边界。

**核心原则：先用查询命令定位，再通过结构化编辑命令修改，并运行适用的检查与测试。**

### 标准流程

```bash
# 搜索 → 修改 → 验证
calcit query search 'symbol' --filter 'ns/def'                    # 1. 定位（输出：@3.2.1 in ...）
calcit tree replace 'ns/def' --path '@3.2.1' --code 'quote |new' # 2. 修改
calcit tree show 'ns/def' --path '3.2.1'                         # 3. 验证（可选）
```

### 三种搜索方式

```bash
calcit query search 'target' --filter 'ns/def'                    # 搜索符号/字符串
calcit query search-expr 'fn (x)' --filter 'ns/def'               # 搜索代码结构
calcit tree replace-leaf 'ns/def' --pattern 'old' --code 'quote |new' # 批量替换叶子节点
```

### 选择定位与修改命令

| 需求 | 先定位 | 再修改或核对 |
| --- | --- | --- |
| 查找符号或字符串 | `query search` | 核对返回的定义 revision 与 Snapshot path 后使用 `tree` |
| 查找表达式结构 | `query search-expr` | 查看匹配的 Cirru 树，再选择结构化编辑 |
| 跨调用点重命名 | `query usages` | 使用 `fix --rule rename-definition-v1` 的预览与应用闭环，核对结果；不要把纯文本替换当成语义重命名 |

这些是工作流选择，不是效率倍率。Agent 完成时间、重试与人工审阅成本须按 #1302 的固定任务和原始记录评估。
