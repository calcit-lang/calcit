---
title: "项目结构：calcit.cirru 与 deps.cirru"
summary: "calcit.cirru 的 EDN 结构说明（:package、:version、:entries、:files、:modules）以及 deps.cirru 的版本管理"
scope: "core"
kind: "reference"
category: "run"
aliases:
  - "project structure"
  - "calcit.cirru"
  - "deps.cirru"
  - "edn structure"
  - "snapshot file"
entry_for:
  - "cr query config"
  - "caps --ci"
---

# 项目结构：`calcit.cirru` 与 `deps.cirru`

## `calcit.cirru` 的 EDN 结构（兼容旧文件名 `compact.cirru`）

Agent 切到新窗口时，优先把 `calcit.cirru`（兼容旧文件名 `compact.cirru`）看成一个"可执行项目快照"，其顶层 EDN 结构通常是：

```cirru.no-check
{}
  :package |my-app
  :version |0.1.0
  :entries $ {}
    :default $ {}
      :mode :js
      :init-fn |app.main/main!
      :reload-fn |app.main/reload!
      :description "|Browser client"
      :modules $ [] |lilac/ |memof/
      :type-slots $ {}
        :dispatch-op |app.schema/Op
    :test $ {}
      :mode :native
      :init-fn |app.test/main!
      :reload-fn |app.test/reload!
      :description "|Test runner"
      :modules $ [] |calcit-test/
      :type-slots $ {}
        :dispatch-op |app.test-schema/TestOp
  :files $ {}
    |app.main $ %{} :FileEntry
      :ns $ %{} :CodeEntry ...
      :defs $ {}
        |main! $ %{} :CodeEntry ...
```

字段职责可以快速记成：

- `:package`：包名边界（影响哪些 namespace 允许被 `cr edit` 修改）。
- `:version`：项目版本，独立于各运行入口。
- `:entries`：全部运行入口；不指定 `--entry` 时使用 `:default`。
- `:mode`：入口的默认运行后端，只接受 `:native` 或 `:js`。
- `:description`：entry 用途的简短语义说明；只提供给人和工具阅读，不影响运行。
- `:type-slots`：当前 entry 的编译期类型绑定（slot 名 → 完整 `namespace/definition` 路径或 `:dynamic`）。各 entry 都保存完整配置，彼此不继承。
- `:files`：源码数据库（namespace → `:ns` + `:defs`；每个定义是 `CodeEntry`，包含 code/doc/examples/schema）。
- `:modules`：加载的外部模块路径（通常来自 `~/.config/calcit/modules/`，目录结尾 `/` 默认补 `calcit.cirru`，并回退到 `compact.cirru`）。

一般避免直接修改 `calcit.cirru` 文件（兼容旧文件名 `compact.cirru`）, 因为可能会导致格式出错整体无法解析, 如果确实认为需要修改, 要确保修改以后立即执行 `cr calcit.cirru edit format` 确保能够正确格式化.

启动解析顺序（实操最常用）：

1. `cr`：使用 `:entries.default`，并按它的 `:mode` 运行。
2. `cr --entry test`：切到 `:entries.test`，并按它的 `:mode` 运行。
3. `cr --init-fn xxx`：覆盖入口函数（常用于测试链路临时指定）。

建议每次开工先跑 3 条，建立项目运行心智：

```bash
cr query config
cr query ns <target-ns>
cr query defs <target-ns>
cr config type-slots
```

Type slot 优先用配置命令维护，避免直接改 snapshot：

```bash
cr config set-type-slot :dispatch-op app.schema/Op
cr config set mode js
cr config set description "Browser client"
cr config set-type-slot --entry test :dispatch-op app.test-schema/TestOp
cr config rm-type-slot :dispatch-op
```

## `deps.cirru` 与运行时快照文件的关系

给 Agent 一个最小心智就够：

- `deps.cirru`：声明"要下载哪些外部模块 + 期望的 calcit 版本"。
- `calcit.cirru`（兼容旧文件名 `compact.cirru`）：声明"运行时要加载哪些模块（`:modules`）+ 项目代码快照（`:files`）"。

常见升级动作（最少命令）：

```bash
# 1) 看本机 CLI 版本
cr --version

# 2) 看依赖是否有更新
caps --ci outdated

# 3) 直接更新 deps.cirru（无交互）
caps --ci outdated --yes

# 4) 下载/同步模块后再编译验证
caps --ci
cr js
```

实践里优先保证两件事一致：

- `deps.cirru` 的 `:calcit-version` 与当前 `cr --version` 不要偏差太大；
- `package.json` 的 `@calcit/procs` 与当前 Calcit 版本链路保持同一代。
