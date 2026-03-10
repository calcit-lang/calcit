# 202602272223 CalcitAgent.md 优化：精简冷僻内容，补充实战示例

## 修改概要

对 `docs/CalcitAgent.md` 进行了三项优化，使文档对 Agent 更加实用：

### 1. 精简 "LLM 辅助：动态方法提示" 一节

**知识点**：低频场景的文档应指向 `cr docs`，而不是在主文档中展开细节。

- 将 4 个内置函数的冗长说明（每个都有 3-4 行 `用法/用途/说明`）压缩为 4 行简要说明
- 末尾加 `cr docs read traits.md` 和 `cr docs search 'trait-call'` 的指引入口
- 减少约 300 字，信噪比更高

### 2. 扩充 "常见错误排查" 一节

**知识点**：错误排查文档需要配有"如何读懂错误"的示例，而不只是一张表格。

变更：
- 新增 **快速诊断流程** 子节：明确排查步骤（`cr query error` → `--check-only` → `cr eval` 隔离验证），并附带 `cr query error` 输出示例（含 unknown symbol 拼写错）
- 扩展错误信息对照表：从 4 条扩展到 11 条，覆盖 `unknown symbol`、`let` 语法、`cannot be used as operator`、`foldl` 参数顺序、`imports` `:require` 前缀格式等高频错误，每条均有解决命令
- 新增 **调试常用命令** 子节：汇总 `cr query error`、`cr eval`、`cr query find`、`cr cirru parse` 等常用排查命令
- 新增 `.calcit-error.cirru` 备份文件的说明（比 `cr query error` 更完整）

### 3. 新增 "🔄 完整功能开发示例" 一节

**知识点**：Agent 最常见的任务是"添加新函数"，需要端到端的完整流程示例。

内容：
- 步骤 1: `cr query ns` / `cr query defs` / `cr query peek` 确认现有代码
- 步骤 2: `cr eval` 快速验证写法（含 `--dep` 加载模块的示例）
- 步骤 3: `cr edit def` 添加新定义
- 步骤 4: `cr edit add-import` 添加 import，`cr tree replace` 在调用方使用新定义
- 步骤 5: `cr edit inc --changed` 触发热更新 + `cr query error` 验证 + `cr --check-only` 整体检查
- 末尾附"常见失误快速修复"（忘记 import、拼写错误、参数顺序错误）

## 关联规则

- 冷僻/参考类内容应精简，提供 `cr docs search / read` 入口
- 高频场景应提供完整示例（搜索→修改→验证→热更新）
- 错误排查文档必须包含错误内容预览，而不只是表格
