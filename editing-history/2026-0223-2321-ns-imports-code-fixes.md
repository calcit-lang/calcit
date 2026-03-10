# 2026-0223 命名空间 imports 操作 Bug 修复

## 背景

通过在 `demos/compact.cirru` 上逐一测试 `cr edit` 的 namespace 相关子命令，发现三处导致 Agent 频繁用错的根本原因。

## 发现的 Bug

### Bug 1：`imports -e 'rule'` 产生平坦结构，与 `extract_require_rules` 不兼容

- **现象**：`imports -e 'respo.core :refer $ sym'` 成功，但之后执行 `add-import` 会丢失已有 imports
- **根因**：`handle_imports` 手工拼接 `ns_code_items`（平坦 list），产生结构：
  ```
  ["ns", "my.ns", ":require", "respo.core", ":refer", "$", "sym"]
  ```
  而 `extract_require_rules`/`build_ns_code` 期望嵌套结构：
  ```
  ["ns", "my.ns", [":require", ["respo.core", ":refer", "$", "sym"]]]
  ```
  导致后续任何依赖 `extract_require_rules` 的操作（`add-import`、`rm-import`）全部解析不到已有规则。
- **修复**：重写 `handle_imports` 的 rules 解析逻辑，统一调用 `build_ns_code` 生成嵌套结构。同时自动区分输入是单条规则（flat array of strings）还是多条规则（array of arrays）。

### Bug 2：`imports -e ':require ...'` 静默产生 `:require :require` 重复

- **现象**：写 `:require respo.core :refer $ sym` 不报错，但生成 `ns my.ns :require :require respo.core ...`
- **修复**：当 Cirru 解析后发现数组第一元素为 `:require` 字符串，立即返回错误消息，引导用户不包含 `:require` 前缀。

### Bug 3：`add-ns -e 'ns WRONG_NAME ...'` 名称不一致静默通过

- **现象**：`cr edit add-ns my.ns -e 'ns wrong.ns ...'` 成功，file-key 是 `my.ns`，但 ns 声明内写的是 `wrong.ns`，导致 `query ns my.ns` 看到内部名称错误
- **修复**：当输入解析为 `ns` 表达式时，校验第二个元素是否与位置参数一致，不一致则 Error。

## 修改文件

- `src/bin/cli_handlers/edit.rs`：`handle_imports`、`handle_add_ns` 函数

## 知识点

- `imports` 命令的 `-e` 输入格式：**不含 `:require` 前缀**，直接是规则体（`src-ns :refer $ sym`）。单条规则传平坦字符串，多条规则用 `-f` 文件（每行一条）或 `-j` JSON 数组（元素为数组）。
- `add-import` 和 `imports` 的格式一致，都是 `src-ns :refer $ sym`（无 `:require` 前缀）。
- `add-ns -e` 中若传完整 `ns` 表达式，内部名称必须与位置参数完全匹配。
- 最佳实践：优先使用 `add-import`（带保护和覆盖控制），`imports` 只在需要全量重置时使用。
