---
title: "开发调试"
summary: "增量开发流程：watcher 监听模式、cr edit inc 增量更新、持久化错误栈与典型终端工作流"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "debugging"
  - "incremental update"
  - "hot reload"
  - "cr edit inc"
  - "watcher"
entry_for:
  - "cr edit inc"
  - "cr -w"
  - "cr js -w"
  - "cr query error"
  - "cr ir"
---

# 开发调试

简单脚本可直接使用 `cr <filepath>` 执行（默认单次）。编译 JavaScript 用 `cr <filepath> js` 执行一次编译。
若需要监听模式，显式添加 `-w` / `--watch`（如 `cr -w <filepath>`、`cr <filepath> js -w`）。

`cr ir` 只输出供编译器调试使用的内部表示，普通项目开发和验证不需要它；明确排查 IR 时再查看 `cr ir --help`。

Calcit snapshot 文件中 config 有 `init-fn` 和 `reload-fn` 配置：

- 初次启动调用 `init-fn`
- 每次修改代码后调用 `reload-fn`

**典型开发流程：**

```bash
# 1. 启动监听模式（用户自行使用）
cr -w        # 解释执行监听模式
cr js -w     # JS 编译监听模式

# 2. 修改代码后触发增量更新（详见"增量触发更新"章节）
cr edit inc --changed ns/def

# 3. 一次性执行/编译（用于简单脚本）
cr             # 执行一次
cr js          # 编译一次
```

## 增量触发更新（推荐）⭐⭐⭐

当使用监听模式（`cr -w` / `cr js -w`）开发时，推荐使用 `cr edit inc` 命令触发增量更新，而非全量重新编译/执行。

**工作流程：**

```bash
# 【终端 1】启动 watcher（监听模式）
cr -w        # 或 cr js -w

# 【终端 2】修改代码后触发增量更新
# 修改定义
cr edit def app.core/my-fn --code 'quote (defn my-fn (x) (+ x 1))'

# 触发增量更新
cr edit inc --changed app.core/my-fn

# 等待 ~300ms 后读取 watcher 持久化的错误栈
cr query error
```

**增量更新命令参数：**

```bash
# 新增定义
cr edit inc --added namespace/definition

# 修改定义
cr edit inc --changed namespace/definition

# 删除定义
cr edit inc --removed namespace/definition

# 新增命名空间
cr edit inc --added-ns namespace

# 删除命名空间
cr edit inc --removed-ns namespace

# 更新命名空间导入
cr edit inc --ns-updated namespace

# 组合使用（批量更新）
cr edit inc \
  --changed app.core/add \
  --changed app.core/multiply \
  --removed app.core/old-fn
```

**查看 watcher 持久化错误栈：**

```bash
cr query error  # 显示 .calcit-error.cirru 最近持久化的错误栈
```

当前命令的 stderr 始终是本次 parse、preprocess、query 或 edit 失败的第一证据；这些失败不保证刷新 `.calcit-error.cirru`。`cr query error` 主要用于读取最近持久化的 runtime/watcher stack；如果它提示 stale，后面的内容可能是旧任务留下的无关错误，不要据此修代码。

即便错误 sidecar 是最新的，它也**不能**证明浏览器 CSS、HTML 属性值、业务数据内容或外部系统配置是"合理的"。像 `|max(...)` 被误写成 `"|max(...)` 这类在 Cirru 层面仍合法的字符串，仍可能在浏览器渲染阶段失效。

**何时使用全量操作：**

```bash
# 极少数情况：增量更新不符合预期时
cr js              # 重新编译 JavaScript
cr                 # 重新执行程序

# 或重启监听模式（Ctrl+C 停止后重启）
cr        # 或 cr js
```

**增量更新优势：** 快速反馈、精确控制变更范围、watcher 保持运行状态
