---
title: "模块装载与库 / Demo 边界"
scope: "core"
kind: "reference"
category: "installation"
aliases:
  - "modules directory"
  - "installed modules"
  - "module docs"
  - "caps"
  - "library modules"
  - "demo modules"
  - "dev-dependencies"
entry_for:
  - "caps install"
  - "calcit docs remote-libs scan-md"
  - "calcit eval --dep"
  - "calcit config add-entry"
---

# 模块装载与库 / Demo 边界

`caps` 根据 `deps.cirru` 下载模块；Calcit 根据所选 Snapshot entry 的 `:modules` 装载源码。
下载依赖和装载源码是两份合同：把模块移到 `:dev-dependencies`，不会自动从 `:modules` 删除它。

## 装载规则

Snapshot 的模块配置示意如下；实际写入使用 `calcit config` 或受保护的编辑事务，不直接编辑 Snapshot 文本：

```cirru.edn
{}
  :entries $ {}
    :default $ {}
      :modules $ [] |memof/calcit.cirru |lilac/
```

通常模块安装在项目 `.calcit/modules/`；全局模块目录为 `~/.config/calcit/modules/`。
用 `calcit config modules --format edn` 查看实际解析结果，不根据目录里已有的文件猜测依赖归属。
以 `/` 结尾的模块路径会补上 `calcit.cirru`；已退役的 `compact.cirru` 会被拒绝并提示迁移。

根项目的 `--entry demo` 只选择根 Snapshot 的入口。模块作为依赖加载时，递归读取它自己的
`entries.default.modules`；不会把根项目的 entry 名传给每个依赖，也不会根据最终调用哪个纯函数来裁剪声明的模块。
因此默认图中缺失的模块仍会报错，即使调用目标没有使用它。不要用空模块或静默跳过缺失文件来修补。

## 库和 Demo 使用不同模块图

一个可复用库可以保留完整 Demo 源码，用现有 named entry 分离两种装载范围：

| 入口 | 模块图与职责 |
| --- | --- |
| `default` | 消费者需要的运行依赖及其传递依赖；init/reload 若要供根项目直接检查或运行，应指向真实的库 smoke 入口，不应仍依赖未装载的 Demo 模块 |
| `demo` | Demo 需要的完整模块图、init/reload、mode、target、feature policy 与 type slots；每个入口独立保存配置，不继承 default |

以仅被 Demo 使用的 UI/Reel 为例：只有确认库的公开函数、schema、宏展开及消费者测试均不需要它们后，
才能将它们移入 `:dev-dependencies`，并仅保留在 Demo 的模块图中。若 Respo 或 JS-FFI 仍被公开库使用，
它们继续留在运行依赖及默认图中。

迁移前先查询真实配置和入口定义：

```bash
calcit config show --format edn
calcit config modules --format edn
calcit query ns
caps tree
caps why 'Respo/respo-ui.calcit'
```

在同一编辑事务中，先用 `config add-entry demo --from default` 保存原完整图，再调整两个入口的
init/reload、mode、target 和模块列表；default 需要新 smoke 定义时，先补齐真实实现与类型合同。
`--from default` 是创建时复制，不是后续继承。事务先 dry-run，审阅后带返回的 scoped revision 应用；
具体操作文件格式见 `calcit docs read edit-tree.md 'Atomic Transactions'`。
不要只从 default 删模块而保留依赖这些模块的入口，也不要为保持检查全绿而删除 Demo。

## 分开验收库消费与完整项目

下面以 `feather.core` 为例；命名空间、定义和 Demo entry 必须以项目查询结果为准：

```bash
# 在库根目录，只检查所选公开命名空间；需要 entry 已声明 target。
calcit --check-only --ns feather.core --format edn
# 显式选择完整图，检查包括 Demo 在内的全部本地定义和严格证明。
calcit --entry demo --check-only --all-defs --format edn
calcit --entry demo fix --workflow strict --verify --format edn
calcit --entry demo test feather.core/icon-color-text --require-match
# 按 Demo entry 的 mode 运行或生成 JS。
calcit --entry demo
```

根项目用 `caps --ci` 安装运行与开发依赖，验收 Demo、附带测试和文档示例。
另建干净消费者，只安装库的运行依赖图，再通过正常的 Calcit 引用或
`calcit eval --dep ./ 'feather.core/icon-color-text :red'` 验证公共入口；此处 `./` 指已准备好的库目录。
同时保留错误参数的拒绝测试，不能因分离模块图而放宽公开类型。

仅在库默认图上检查公开 namespace，不等于整个源码库通过 `--all-defs` 或完整 strict。
后两者会检查本地 Demo 定义，应在依赖完整的 Demo entry 上执行。附带测试、examples、Markdown
有自己的依赖范围，也必须实际执行，不能把零匹配或尚未运行的路径当成通过。

JS 生成还要实际验证模块提供的资源。`:ffi :js :file` 的资源路径属于定义所在模块的源码目录，
不会因为调整 entry 或依赖分组就改为根项目目录；沿用现有声明和正常 Calcit 引用，
不要复制片段、拼 npm 路径或添加应用专属 resolver 来掩盖装载问题。参见 [JS 互操作](../features/js-interop.md)。
