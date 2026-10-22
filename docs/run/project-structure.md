---
title: "项目结构：calcit.cirru 与 deps.cirru"
summary: "calcit.cirru 的 EDN 结构说明（:package、:entries、:files、:modules）以及 deps.cirru 的版本管理"
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
  - "calcit query config"
  - "caps --ci"
---

# 项目结构：`calcit.cirru` 与 `deps.cirru`

## Namespace 的 require 写法

`:require` 下面每行已经是 Cirru 语法列表，`:refer` 后面用 `$` 形成名称列表，
两处都不需要运行时 List 构造标记 `[]`：

```cirru.no-check
ns app.main $ :require
  respo.core :refer $ render! clear-cache!
  respo.cursor :refer $ update-state-tree
  respo-ui.router :as router
```

旧的 `[] respo.cursor :refer $ [] update-state-tree` 仍按原导入规则解析，但加载与
import 编辑会在 stderr 给出 warning，标明 namespace、rule 编号和标记位置。
这是写法提示，不增加类型错误，也不改变 alias、refer、default 或重复绑定的解析语义。
更新时用 `calcit edit add-import <namespace> --overwrite` 替换已存在的规则，
多规则修改使用 `edit transaction` 的 dry-run 与 revision 守卫。

### 内建名称与模块实现

裸内建函数名优先于 `:refer`，因此 `generate-id!` 即使出现在模块导入中，裸调用仍使用
`calcit.core/generate-id!`。要调用模块自己的同名实现，使用 alias 或完整限定名：

```cirru.no-check
ns app.main $ :require
  project.ids :as ids

defn make-id ()
  ids/generate-id!
```

0.32.0 的预处理在使用内建函数时检查同名显式导入的 Calcit 目标是否存在。
如果 namespace 或 definition 不存在，会报告声明的目标和实际选中的内建函数，
严格检查及 JS 生成失败；删除过期的 import，或修正来源后改用限定名。
读取器已将源码内建名称转为 Proc，因此该诊断定位到所属定义，不声称定位到具体 token。
有效导入只检查定义存在，不预处理或运行被内建名称遮蔽的模块实现。

限制：这不是未使用 import 的全量审计，也不验证 JavaScript 模块的 exports；宿主命名空间与已注册宿主过程仍按原 FFI 边界处理。

## `calcit.cirru` 的 EDN 结构

Agent 切到新窗口时，把 `calcit.cirru` 看成一个"可执行项目快照"，其顶层 EDN 结构通常是：

```cirru.no-check
{}
  :package |my-app
  :entries $ {}
    :default $ {}
      :mode :js
      :init-fn 'app.main/main!
      :reload-fn 'app.main/reload!
      :description "|Browser client"
      :modules $ [] |lilac/ |memof/
      :type-slots $ {}
        :dispatch-op |app.schema/Op
    :test $ {}
      :mode :native
      :init-fn 'app.test/main!
      :reload-fn 'app.test/reload!
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

- `:package`：包名边界（影响哪些 namespace 允许被 `calcit edit` 修改）。
- `:version`：不再属于推荐的 Snapshot 配置；旧 Snapshot 可能仍带有该字段，但 `caps` 不会读取它。项目版本请维护在 `deps.cirru :version`（用 `caps version get/set/bump` 管理）。
- `:entries`：全部运行入口；不指定 `--entry` 时使用 `:default`。
- `:mode`：入口的默认运行后端，只接受 `:native` 或 `:js`。
- `:description`：entry 用途的简短语义说明；只提供给人和工具阅读，不影响运行。
- `:type-slots`：当前 entry 的编译期类型绑定（slot 名 → 完整 `namespace/definition` 路径或 `:dynamic`）。各 entry 都保存完整配置，彼此不继承。
- `:files`：源码数据库（namespace → `:ns` + `:defs`；每个定义是 `CodeEntry`，包含 code/doc/examples/schema）。
- `:modules`：加载的外部模块路径（通常来自项目 `.calcit/modules/`，目录结尾 `/` 补 `calcit.cirru`；旧 `compact.cirru` 模块会被拒绝）。

程序 Snapshot 只通过 `calcit config` / `edit` / `tree` / `cursor` 修改，写入前读取
`calcit docs agents --contract`。`edit format` 用于规范化序列化结果，不能代替受保护的结构化编辑。

启动解析顺序（实操最常用）：

1. `calcit`：使用 `:entries.default`，并按它的 `:mode` 运行。
2. `calcit --entry test`：切到 `:entries.test`，并按它的 `:mode` 运行。
3. `calcit --init-fn xxx`：覆盖入口函数（常用于测试链路临时指定）。

建议每次开工先跑 3 条，建立项目运行心智：

```bash
calcit query config
calcit query ns <target-ns>
calcit query defs <target-ns>
calcit config type-slots
calcit config show --format edn
calcit config modules --format edn
calcit config type-slots --format edn
```

Calcit 自有自动化应使用三个 config 查询的 `--format edn`，不要解析面向人的输出；对接 JSON-only
工具时才显式选择 `--format json`。两种结构化格式的成功与失败结果都只输出一个 envelope，
包含版本、命令、数据、诊断和 revision；EDN 键为 `:schema-version` 等 kebab-case 名称，
JSON 对应 `schema_version` 等兼容名称。失败仍以非零状态退出。`config show` 的全部入口结果按名称排序，
`config modules` 会为每个声明路径报告
`loaded`/`failed` 和解析出的 package。

Type slot 优先用配置命令维护，避免直接改 snapshot：

```bash
calcit config set-type-slot :dispatch-op app.schema/Op
calcit config set mode js
calcit config set target browser
calcit config set description "Browser client"
calcit config add-entry test --mode native --init-fn app.test/main! --reload-fn app.test/reload!
calcit config set --entry test target node
calcit config set-type-slot --entry test :dispatch-op app.test-schema/TestOp
calcit config rm-type-slot :dispatch-op
```

确认命名入口已经废弃后，用 `calcit config unset entry --entry <name>` 删除整个入口配置；
命令只删除 Snapshot 中的入口配置，不删除对应源码或已安装模块目录。必须显式传 `--entry`，不能删除
`default`，不存在的入口会报错。被 verification profile 引用的入口也会拒绝删除，并列出相关 profile；
不会静默改写验收范围。先核对 `config show --format edn` 中的入口、依赖及项目文档，
不要为了跳过失败验收而删除仍有用途的测试入口。

需要预览和 revision 守卫时，沿用同一个编辑事务：

```bash
calcit edit transaction --code '[] $ [] |config |unset |entry |--entry |obsolete' --dry-run --format edn
# 审阅预览后，使用返回的 scoped-revision；只守卫删除的入口，其他定义的独立修改仍可保留。
calcit edit transaction --code '[] $ [] |config |unset |entry |--entry |obsolete' --expect-revision <revision> --format edn
```

升级 CLI 后请重新预览事务，不复用旧工具生成的临时 revision。

旧项目的包名与本地命名空间前缀不一致时，可用 `calcit config set package app` 修正
Snapshot 的项目级 `:package`。包名由非空 symbol 段组成，用点分隔；此命令只修改元数据，
不重命名命名空间、定义、引用或入口，也不写入依赖模块。先查询本地源码，确认新的归属前缀；
该命令不接受 `--entry`。

需要预览和 revision 守卫时，复用编辑事务：

```bash
calcit edit transaction --code '[["config","set","package","app"]]' --dry-run --format edn
# 确认预览后，使用返回的 scoped-revision 应用同一事务：
calcit edit transaction --code '[["config","set","package","app"]]' --expect-revision <revision> --format edn
```

## Architecture plans and local Agent state

Architecture plans that belong in version control should live under
`docs/architectures/<feature>.cirru`. They describe a feature as a flat
Cirru EDN `:definitions` map plus typed anonymous-enum edges, and can be
reviewed independently from the generated Snapshot stubs:

```bash
calcit calcit.cirru edit scaffold \
  --file docs/architectures/order-submission.cirru \
  --dry-run --format edn
calcit calcit.cirru edit scaffold \
  --file docs/architectures/order-submission.cirru \
  --expect-revision md5:...
```

`--dry-run` is read-only. An apply creates only missing `:ensure` definitions,
preserving existing code/doc/schema and reporting compatible reuse or conflicts.
Generated function definitions carry a `:scaffold` tag and `todo!` body so they
remain visible to static checks and can be assigned as work items.

`.calcit/` remains local transient state for cursor, error, and snippets. Do
not store architecture plans there; do not commit cursor sidecars with the
project source.

## `deps.cirru` 与运行时快照文件的关系

给 Agent 一个最小心智就够：

- `deps.cirru`：声明"要下载哪些外部模块 + 期望的 calcit 版本 + 项目发布版本（`:version`）"。
- `calcit.cirru`：声明"运行时要加载哪些模块（`:modules`）+ 项目代码快照（`:files`）"。

版本号只以 `deps.cirru :version` 为权威，用 `caps version get/set/bump` 管理；`calcit.cirru :version`
不会作为 `caps` 的回退来源。`calcit config version` 已在 0.31.0 删除，`calcit config set version` 会报错，请改用 `caps version`。

常见升级动作（最少命令）：

```bash
# 1) 看本机 CLI 版本
calcit --version

# 2) 看依赖是否有更新
caps --ci outdated

# 3) 直接更新 deps.cirru（无交互）
caps --ci outdated --yes

# 4) 下载/同步模块后再编译验证
caps --ci
calcit js
```

实践里优先保证两件事一致：

- `deps.cirru` 的 `:calcit-version` 与当前 `calcit --version` 不要偏差太大；
- `package.json` 的 `@calcit/procs` 与当前 Calcit 版本链路保持同一代。
