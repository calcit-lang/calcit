---
title: "Calcit 项目升级手册（Respo / Lilac）"
summary: "项目升级流程：依赖与工具链同步、entries/type-slots 迁移、静态质量审计、CI 与消费者回归"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "upgrade"
  - "dependency migration"
  - "respo upgrade"
  - "lilac upgrade"
id: core/run/upgrade
related:
  - core/run/library-quality
  - core/run/entries
  - core/features/static-analysis
---

# Calcit 项目升级手册（Respo / Lilac）

本手册面向需要从旧版 Calcit 逐步迁移到当前版本的项目。它不能保证旧代码无需修改就一次通过；
它保证的是：先保留可回滚基线，再用 Calcit CLI 把依赖、Snapshot、配置、类型和行为问题分层暴露，
每一层通过后再收紧下一层，避免把所有失败混在一次升级里。类库/module 发布前的完整证据矩阵见
[Calcit 类库项目验收与质量门禁](library-quality.md)。

适用对象：通过 Calcit CLI 运行并产出 JS 的项目（例如 Respo）。

升级完成的标准不是“`caps upgrade --all` 执行成功”，而是：

- 新版 `calcit` / `caps` 已固定到本地和 CI，且 `deps.cirru`、`@calcit/procs` 版本链路一致；
- 每个声明支持的 entry 都通过 `--check-only`，并完成对应 native / JS 行为测试；
- `check-types`、`weak-types`、`deprecated` 已生成可复查报告，存量债务有 baseline，新增债务被阻断；
- examples、Markdown 示例、项目测试与真实消费者回归覆盖了公开能力。

---

## 1）升级前检查位置

### 先建立可回滚基线

不要在未提交的业务修改上直接同时升级 CLI、依赖、Snapshot 和类型。先确认工作区状态，创建升级分支，
并记录旧工具链下确实成功的命令、entry 和构建产物：

```bash
git status --short
git switch -c upgrade/calcit-latest
calcit --version
calcit compact.cirru                # 替换为旧项目原本可成功执行的入口命令
yarn test                       # 替换为项目原有的测试/构建命令
```

如果旧项目当前就不能通过原有测试，应先把失败记录为已知基线；不要把它误归因于新版类型系统。
Snapshot 文件迁移、依赖升级和类型修复建议分别提交，任何阶段都能单独回退和比较。
不要要求旧 CLI 支持本文后续介绍的所有新版子命令；这些校验应在 Step A 更新工具后再执行。

升级前先检查以下文件与配置是否齐全：

- 运行入口：`calcit.cirru`（兼容旧文件名 `compact.cirru`）
  - `:entries.default`（默认入口及 `:mode`）
  - `:entries.<name>`（额外入口及各自 `:mode`）
- 命令入口：`README`、项目脚本、CI workflow
- Node 工具链：`package.json`、`yarn.lock`、Corepack/Yarn 版本
- 注意 git fetch 检查最新历史, 避免基于老版本操作导致变更冲突
- 依赖边界：运行/编译期需要的模块放在 `:dependencies`；只供当前项目测试、examples、文档检查和维护脚本使用的模块放在 `:dev-dependencies`
- 结构化编辑优先使用 `calcit edit` / `calcit tree`；若直接改过 `calcit.cirru`（或旧文件名 `compact.cirru`），提交前执行一次 `calcit calcit.cirru edit format`
- 静态质量基线：`check-types`、`weak-types`、公开 namespace examples 与 Markdown 示例

### 快照文件迁移说明

以前的 Calcit 项目使用两套文件：

- `calcit.cirru` — 存放完整 AST 快照，内容包含全部编译信息（带所有代码位置、类型标注等）
- `compact.cirru` — 存放精简代码，是人工读写的主要文件

当前推荐去掉旧的双文件模式，把精简 Snapshot 直接保存在 `calcit.cirru` 中，方便 `calcit` 命令直接读取使用。
如果两个文件同时存在，不要仅凭文件名猜测哪个是有效源码；先在旧版本下分别检查 Git 历史、文件体积、
项目脚本和实际运行入口。确认 `compact.cirru` 是当前可运行的精简 Snapshot 后再迁移：

1. 确认 `compact.cirru` 是项目实际精简化代码，`calcit.cirru` 是完整 AST 快照（差异通常很大）
2. 先在独立提交或临时分支中将 `compact.cirru` 复制/重命名为 `calcit.cirru`，不要和业务逻辑修复混在一起
3. 执行 `calcit calcit.cirru edit format`，审阅 diff，再对新的 `calcit.cirru` 运行 `--check-only` 和原有 entry/构建测试
4. 新旧入口行为一致后再删除旧文件并提交；后续所有 `calcit` 命令都显式基于单一的 `calcit.cirru`

如果新版 `calcit` 连旧的完整 `calcit.cirru` 都无法反序列化，就不能先对它执行 `edit format`。先确认旁边的
`compact.cirru` 能在旧工具链运行，并从 Git 状态/历史确认它是最后的有效精简源码；然后用可恢复的步骤重建：

```bash
cp calcit.cirru calcit.full-snapshot.backup.cirru
cp compact.cirru compact.migration-backup.cirru
$EDITOR compact.cirru
# 在每个 ns 规则中将 :require-macros 改为 :require，并确认不再有旧 clause
if rg -q ':require-macros' compact.cirru; then
  echo '请先逐个迁移 compact.cirru 中的 :require-macros 规则'
  exit 1
else
  status=$?
  if [ "$status" -ne 1 ]; then
    echo '无法检查 compact.cirru 中的 :require-macros 规则'
    exit "$status"
  fi
fi
cp compact.cirru calcit.cirru
calcit calcit.cirru edit format
git diff -- calcit.cirru
calcit calcit.cirru --check-only
```

若 `rg` 没有匹配，才继续复制和格式化；如果仍有匹配，应逐个编辑 namespace 规则，而不是用全局替换，
以免改动代码字符串中的同名文本。不要删除备份或旧文件，直到所有 entry 与原有 native/JS 测试均通过。现在反序列化错误会带上失败的
Snapshot 路径；如果同目录存在 `compact.cirru`，还会直接给出上述恢复方向。

旧 namespace 里的 `:require-macros` 也必须在 Snapshot 规范化前处理。宏与普通值现在共用
`:require` 规则，例如把：

```cirru.no-check
ns app.main $ :require-macros
  legacy.macros :refer $ defcomp
```

改为：

```cirru.no-check
ns app.main $ :require
  legacy.macros :refer $ defcomp
```

遇到旧写法时，加载阶段会明确报告 `:require-macros` 迁移提示，而不是只返回笼统的 invalid `ns` form。

> `calcit` 仍然读取 Snapshot，只是当前约定把精简 Snapshot 直接命名为 `calcit.cirru`；不要把它理解为
> “不再依赖 Snapshot”。`calcit edit` / `calcit tree` 会直接修改该文件。确认迁移完成后，可以移除旧的
> `.gitattributes` generated 标记和过时的双文件生成脚本；不要把仍可能承载源码的文件直接加入 ignore。

---

## 2）标准升级流程（建议顺序）

下面流程按“先确认版本，再对齐工具链，再更新依赖，最后按 CI 链路验证”的顺序执行。

### 快速命令清单

```bash
# 安装/更新用户工具；本地和 CI 必须使用同一版本链路
cargo install calcit --bin calcit --bin caps --force
calcit --version
caps upgrade --all
caps
corepack enable
corepack prepare yarn@4.12.0 --activate
yarn install
yarn install --immutable
calcit calcit.cirru edit format
calcit calcit.cirru --check-only
calcit calcit.cirru --warn-dyn-method --check-only
calcit calcit.cirru analyze deprecated --summary-only --format json
calcit calcit.cirru analyze weak-types --intent unresolved,declared-optional --summary-only --format json
calcit calcit.cirru
yarn vite build --base=./
```

说明：`yarn install` 只在 lockfile 迁移或依赖变更时需要；平时可直接从 `yarn install --immutable` 开始。

### Step A：确认 Calcit CLI 版本

```bash
cargo install calcit --bin calcit --bin caps --force
calcit --version
caps --help
```

说明：`calcit` 和 `caps` 是同一 Calcit 发布链路中的用户工具，应一起更新。不要先用旧 `caps` 改依赖，
再用新 `calcit` 判断结果；也不要只更新本机而让 CI 继续安装旧版。若团队通过其他受控方式分发二进制，
使用该方式即可，但要记录实际版本，并确认 `caps --help` 已包含项目需要的新选项。

> ⚠️ CI 中 `calcit`/`caps` 的项目版本来自 `deps.cirru` 的 `:calcit-version`。新 workflow 使用 `calcit-lang/setup-calcit@v1` 安装该版本，不要再在 workflow 重复传 `version`。Action 会对新 release 临时提供 `cr -> calcit` 兼容链接；对旧 release 则回退到 `cr` asset 并暴露 `calcit`。新命令统一写 `calcit`。已发布的 `calcit-lang/setup-cr` tag 继续支持旧项目；GitHub Actions 不会为 Action 仓库改名重定向，因此迁移必须显式替换 `uses:`。详见 [GitHub Actions](../installation/github-actions.md)。

### Step B：先对齐项目版本与 Node 工具链

重点先检查并对齐以下几处：

- `deps.cirru` 里的 `:calcit-version`
- `package.json` 里的 `@calcit/procs`
- `package.json` 里的 `packageManager`
- `.yarnrc.yml` 是否需要 `nodeLinker: node-modules`
- `.gitignore` 是否已忽略 `.yarn/*.gz`（避免 Yarn 压缩状态文件入库）

先把这些基础版本与工具链约定对齐，再继续更新依赖，能减少后面重复改 lockfile 或 CI 的次数。

非常旧的项目如果仍使用 `package.cirru`，先把它重命名为 `deps.cirru` 并单独提交；新版 `caps`
会拒绝旧文件名并给出迁移提示。不要同时保留两份依赖清单，否则项目脚本和维护者可能更新不同文件。

更新依赖前先读取当前 Agent/CLI 指南，避免沿用旧命令边界：

```bash
calcit docs agents --full
```

### Step C：检查并更新 `deps.cirru`

先审计依赖分组。新版 `caps` 对根项目同时安装 `:dependencies` 和
`:dev-dependencies`，但递归解析某个依赖模块时只读取它的 `:dependencies`，不会把该模块
自己的开发依赖带入消费者。升级旧项目时，应把测试、examples、文档验证与维护工具专用模块
迁到 `:dev-dependencies`，避免递归依赖图继续无边界扩张：

```cirru
{} (:calcit-version |0.13.13)
  :dependencies $ {} (|calcit-lang/respo.calcit |0.16.67)
  :dev-dependencies $ {} (|calcit-lang/calcit-test |0.1.0)
```

可以用 `caps add --dev <org/repo>@<ref>` 和 `caps remove --dev <org/repo>` 管理开发依赖。
同一个仓库不要以不同 ref 同时出现在两个分组；新版会直接拒绝这种歧义配置。

```bash
caps upgrade --all
```

说明：`caps upgrade --all` 会检查 `:dependencies` 与根项目的 `:dev-dependencies`，更新
对应分组中的依赖版本与 `:calcit-version`；如果确实发生升级，还会顺带执行一次
`yarn up @calcit/procs`，把 JS 运行时包同步到当前 Calcit 版本链路。

如果依赖清单本来已经是最新、但 `package.json` 中的 `@calcit/procs` 仍旧，`caps upgrade --all`
可能没有产生更新动作。此时应显式执行 `yarn up @calcit/procs`，审阅 `package.json` / `yarn.lock`，
并确认安装后的包版本与当前 Calcit 发布链路相符；不要只根据 caps 的 “Already up to date” 判断
JS runtime 已同步。

如果你只想批量把旧版本提升到最新标签，也可以继续用：

```bash
caps outdated --yes
```

这个命令只更新 `deps.cirru`，不触发 `yarn up @calcit/procs`。

### Step D：同步模块内容

```bash
caps
caps tree
caps status
caps verify
```

说明：这一步才会按当前 `deps.cirru` 下载/同步模块内容。根项目的两个依赖分组都会安装，
依赖模块的 `:dev-dependencies` 会在递归解析中排除。`tree` 用于审阅最终递归图，`status` 检查
项目链接和期望版本，`verify` 进一步检查 immutable store、源码修改和 native 构建收据。分支 ref
或版本冲突在迁移期可以先作为明确告警处理；准备固定 CI 时再用 `caps --strict --ci` 阻断这些告警。

### Step E：用 Yarn Berry 安装并校验

```bash
corepack enable
corepack prepare yarn@4.12.0 --activate  # 示例；优先采用 packageManager 固定的版本
yarn --version
yarn install --immutable
```

说明：团队若习惯 Yarn Berry，建议固定 `packageManager` 并使用 `--immutable` 做一致性校验。

如果项目仍依赖 `node_modules` 目录解析，还应补一个 `.yarnrc.yml`：

```yaml
nodeLinker: node-modules
```

### Step F：从 CI workflow 和 package.json 提取检查命令并本地先跑

先看 `.github/workflows/` 里实际执行了哪些命令，再看 `package.json` 里是否有额外构建脚本，然后按同顺序在本地跑一遍。

常见链路例如：

```bash
caps && yarn install --immutable
calcit calcit.cirru --check-only
calcit calcit.cirru
calcit calcit.cirru --entry <entry-name>
calcit calcit.cirru js && yarn vite build --base=./
```

`calcit calcit.cirru` 默认单次执行并选择 `entries.default`；指定 entry 时用 `--entry <name>`。entry 的 `:mode` 已决定 native 运行或 JS 生成，项目脚本与 CI 应优先依赖该配置。只有热更新才加 `-w` / `--watch`；显式 `js` 是兼容/定向 codegen 覆盖，不是每个 JS 项目的必需写法。

`--check-only` 会同时预处理所选 entry 的 init/reload 定义，因此可以发现遗留的 `:reload-fn`
已不存在、调用参数/返回值不匹配、Struct/Enum 字段错误、trait 实现不完整等问题。它在预处理产生
warning 时会非零退出，是类型逐渐严格过程中的主要编译门禁。named entry 不继承 default 配置，
所以必须逐个执行，不能只验证 default：

```bash
calcit calcit.cirru --check-only
calcit calcit.cirru --entry test --check-only
calcit calcit.cirru --entry production --check-only
```

旧项目若一次出现大量错误，按“配置/缺失定义 → deprecated API → nominal data/trait → 函数参数和
返回值 → dynamic/nil debt”的顺序修复。每清完一类就提交并重跑所有 entry，避免用
`&core:ignore-type-warning`、`--skip-arity-check` 或批量改成 `'Dynamic` 掩盖迁移问题。

`--check-only` 的范围是所选 entry 可达的预处理路径，不等于“仓库中每个公开定义都已检查”。
未被应用入口调用的类库 API 应由 definition-attached tests、`check-examples` 或真实消费者覆盖；
不要因为 default entry 通过就跳过公开 namespace 和其他 entry。

如果 `package.json` 里有编译、构建、测试相关脚本，也应本地执行一遍；没有额外脚本可跳过。若项目直接通过 Vite 构建，可执行：

```bash
yarn up vite
yarn vite build --base=./
```

说明：若项目依赖 Vite，升级时建议显式执行一次 `yarn up vite`，并重跑构建确认兼容性。

例如还有：

```bash
yarn <script-name>
```

目标：把 CI 会跑的命令和项目脚本都在本地提前验证，减少合并后失败概率。

---

## 3）近期项目结构与类型迁移

这一节必须在 Step A–E 已完成、项目确实使用新版 `calcit` 和新版依赖后执行。先运行一次
`calcit calcit.cirru edit format` 并单独审阅/提交规范化 diff，再根据 `--check-only` 和静态报告逐类
修复语义问题；不要让自动格式化与大规模业务修复混在同一个不可审阅的提交中。

### 3.1 类型标注语法规范化

新版本把 schema 类型的推荐写法统一为 quoted symbol：`'String`、`'Number`、`'List`、`'Ref`、
`'Fn` 和 `'Dynamic`。旧的 `:string`、`:number`、`:list`、`:ref`、`:fn`、`:dynamic` 仍可加载，
以便平滑升级；普通 tag 数据（例如 enum variant `:ok`、struct field key 和 schema 的
`:return`/`:kind` key）不会被改变。

```bash
calcit calcit.cirru edit format
git diff -- calcit.cirru
calcit calcit.cirru --check-only
```

`edit format` 只改写 schema、`hint-fn`、`assert-type`、`unsafe-coerce`、`defstruct` 和 `defenum`
等类型位置，并将 entry schema 重新序列化为 canonical symbols；它不会猜测或加强实际类型契约。
提交前检查 diff，尤其是具有手写 tag 数据的宏或 DSL。

### 3.2 Struct / Enum 数据模型命名迁移

新数据模型明确区分定义和值：`defstruct` 返回 `StructDef`，`defenum` 返回 `EnumDef`；实例类型
分别是 `Struct` 和 `Enum`。没有具名定义的临时值使用 `%{} _ ...` 和 `%:: _ ...`。旧的
record / tuple 公开名称会产生 `W_REMOVED_DATA_API`，诊断中同时给出替代写法；新 Snapshot 不再
写出 `:record`、`:tuple`、`Record` 或 `Tuple`。

常用迁移如下：

| 旧写法 | 新写法 |
| --- | --- |
| `record?` | `struct?`（值）或 `struct-def?`（定义） |
| `tuple?` | `enum?`（值）或 `enum-def?`（定义） |
| `record-struct` | `struct-definition` |
| `tuple-enum` | `enum-definition` |
| `record-with` / `record-match` | `struct-with` / `struct-match` |
| `&record:*` / `&tuple:*` | 对应的 `&struct:*` / `&enum:*`；定义元数据使用 `&struct-def:*` / `&enum-def:*` |

Struct 字段是定义的一部分，因此已知 struct 上的 `get`、`:field` 和 `.field` 直接返回字段声明
类型，不再自动包装 `Option<T>`。不存在的字段会在静态检查阶段报告，运行期也会抛出普通错误；
升级业务代码时应删除这类访问后的 `.unwrap`。Map 等动态容器的访问仍返回 `Option<T>`，
`get-in` 也继续保留可失败路径语义，不能批量删除其 unwrap。

推荐先执行 `calcit calcit.cirru --check-only`，按诊断逐项替换，再运行完整 JS 回归。不要先全局删除
`.unwrap`；只处理接收者已被推断为 Struct 且字段在 `defstruct` 中声明的访问。

#### Option 返回 API 对照

以下 API 不再用 `nil` 或 `-1` 表示缺失。把结果直接传给算术、字符串或集合函数时，诊断会显示完整的
`Option<T>` 推断类型，并建议用 `option:unwrap-or` 或 `tag-match` 显式处理：

| API | 当前返回类型 | 迁移注意点 |
| --- | --- | --- |
| `find-index` | `Option<Number>` | 不再用 `-1`；索引运算前先处理 `%none` |
| `first` / `last` | `Option<T>` | 空集合和空字符串可能没有元素 |
| `nth` | `Option<T>` | 越界是 `%none`；不要把结果直接当元素值 |
| `get` | `Option<T>` | Map/List 等可缺失查找返回 Option；已知 Struct 的声明字段直接返回字段类型 |
| `get-in` | `Option<T>`（开放动态路径常为 `Option<Dynamic>`） | 任一路径缺失都是 `%none` |
| `get-env` | `Option<String>` | 未设置的环境变量是 `%none` |

```cirru
let
    xs $ [] 1 2 3
    predicate $ fn (x) = x 2
    idx $ find-index xs predicate
    safe-idx $ option:unwrap-or idx 0
  &+ safe-idx 1

match (get-env |APP_MODE)
  (:some mode) (println mode)
  (:none) (println |development)
```

上例使用推荐的原生 `match`；维护旧代码时也可以把同一组分支头替换为 `tag-match`，两者都能处理
`Option` 的 `:some` / `:none`，但 `match` 额外提供穷举检查。

只有业务语义确实有合理默认值时才用 `unwrap-or`；需要区分“缺失”和“存在”时保留两个分支。

#### `.trim` / `.blank?` 接收者迁移

`.trim` 与 `.blank?` 只对静态推断为 String 的接收者可用。若错误写成 `unknown method .trim for map`，
重点不是给 Map 增加方法，而是先修正数据流：当前接收者已被推断成 Map。可将接收者收紧/转换为 String，
或改成 `(trim receiver)` / `(blank? receiver)` 获取直接的参数类型诊断。新的 unknown-method 错误会同时写出
实际接收者类型和这两个函数形式；不要用 `unsafe-coerce` 掩盖业务数据类型错误。

### 3.3 统一 entries

顶层 `:configs` 是兼容输入，不应继续作为新配置写法。执行：

```bash
calcit calcit.cirru edit format
calcit calcit.cirru config show
```

format 会把旧 `:configs` 迁移为 `:entries.default`，并输出 `W_LEGACY_CONFIG`。随后应审阅：

- 每个 entry 都有明确的 `:mode :native` 或 `:mode :js`。
- named entry 是完整配置，不继承 default 的 modules/type slots。
- `:init-fn` / `:reload-fn` 使用 definition symbol，而不是继续新增字符串值。
- entry 用 `:description` 说明用途，便于维护者和 Agent 选择正确入口。

### 3.4 Type slots

类库用 `deftype-slot` 声明由应用提供的编译期类型时，每个使用该 slot 的 entry 都要单独绑定：

```bash
calcit calcit.cirru config type-slots
calcit calcit.cirru config set-type-slot :dispatch-op app.schema/DispatchOp
calcit calcit.cirru config set-type-slot --entry test :dispatch-op app.test-schema/TestDispatchOp
```

未绑定 slot 会回退到 `:dynamic`，可能让 callback 检查和 method specialization 失去静态证据。升级后应逐 entry 检查，而不是只验证 default。

### 3.5 旧 schema 与动态类型

`:any` 只保留为 `:dynamic` 的兼容拼写；新 schema 不应继续引入。`edit format` 会输出 `W_LEGACY_ANY` / `W_DYNAMIC_TYPE_DEBT`，但不会猜测并自动改写类型关系。执行：

```bash
calcit calcit.cirru analyze check-types --summary-only
calcit calcit.cirru analyze weak-types \
  --only schema-dynamic,unresolved-type-slot,code-dynamic,code-nil \
  --intent unresolved,declared-optional \
  --summary-only
calcit calcit.cirru analyze deprecated --summary-only
```

有命中时去掉 `--summary-only` 查看 definition、Snapshot path、impact、suggestion 和 deprecated
目标文档。`check-types` 会把缺失或部分 schema（包括没有元素类型的 List/Map/Ref）列出来；
`weak-types` 同时区分 unresolved dynamic、unbound type slot、明确 JS FFI 边界、Unit nil 和旧 Optional 兼容债务；
`deprecated` 按调用位置指出已废弃 API。输入/输出共享类型时用 `:generics`，只约束能力时用 trait
`:where`，collection/ref 保留类型参数，有限异构值使用 enum。真正的 JS FFI 边界应显式标记
`:features $ #{} :js-ffi`，并在进入 typed code 前 validate/convert。无返回值使用 `Unit`；业务缺失
使用 `Option`，需要错误信息时使用 `Result`，不要让旧 `Optional<T>` 或裸 `nil` 无限保留。

再对每个 entry 开启动态方法告警：

```bash
calcit calcit.cirru --warn-dyn-method --check-only
calcit calcit.cirru --entry test --warn-dyn-method --check-only
```

这个开关会暴露无法静态专门化的方法调用和未类型化的动态接收者。它不是第一步：先修复普通
`--check-only`，再打开额外告警，否则旧项目的初始噪音可能掩盖真正的配置和 API 错误。

### 3.6 存量项目的类型收紧策略

三类命令的退出语义不同，不能只看命令是否成功：

- `--check-only` 和实际 native/JS codegen：预处理错误或 warning 会阻断并非零退出，必须修到通过；
- `check-examples`、`docs check-md` 和 `calcit test`：所选示例/测试失败时阻断；测试应加
  `--require-match`，避免过滤条件拼错后“零测试通过”；
- `check-types`、`weak-types`、`deprecated`：是静态定位报告，有命中不等于非零退出；`analyze quality`
  聚合它们的发布指标，并按零目标或已审阅 baseline 返回失败退出码。

老项目不必在第一次升级提交中把所有历史 dynamic 清零，但必须先让报告可重复，再阻止新增债务：

1. 运行 `analyze quality --write-baseline <file>` 生成原生 baseline，人工审阅后与 Calcit 版本、Snapshot revision 一起记录；baseline 本身保存 scope、汇总指标和每个 definition 的预算；
2. 先要求 `--check-only`、测试和行为构建全绿；
3. CI 拒绝 unresolved dynamic、nil/Optional debt 或 deprecated call 数量上升；
4. 按模块降低 baseline，降到 0 后改为零容忍；baseline 只能下降，不能无说明地更新；
5. 对确实动态的 JS FFI 边界显式声明 `:features $ #{} :js-ffi`，不要用 ignore warning 伪造通过。

baseline 不要只保存一个总数。类型覆盖至少比较 `levels.none` 和
`levels.none + levels.partial`（未完全覆盖总数）：`none` 变成 `partial` 是进步，不应因为
`partial` 单项上升而失败。弱类型则分别比较 `kinds.schema-dynamic` / `unresolved-type-slot` / `code-dynamic` / `code-nil`
和 `intents.declared-optional`，再比较 `deprecated` 的 `summary.calls`。否则一种债务增加、另一种
减少时，相同的总数会掩盖回归。

新项目直接执行零容忍门禁：

```bash
calcit calcit.cirru analyze quality
```

存量项目先审阅现状并生成原生 baseline，再在 CI 中执行比较：

```bash
calcit calcit.cirru analyze quality --write-baseline config/calcit-quality.json
calcit calcit.cirru analyze quality --baseline config/calcit-quality.json
```

原生 baseline 记录 scope、汇总指标和每个 definition 的独立预算。新增 definition 默认预算为零；
一个 definition 的改善不能掩盖另一个 definition 的回归。`--write-baseline` 会原子写入文件，
但 baseline 仍需人工审阅并随仓库提交，每次提高都要在 PR 中解释。

例如把首次审阅后的上限提交为 `config/calcit-upgrade-baseline.json`：

```json
{
  "typeNone": 4,
  "typeNotFull": 22,
  "schemaDynamic": 21,
  "codeDynamic": 0,
  "codeNil": 22,
  "unresolved": 43,
  "declaredOptional": 0,
  "deprecatedCalls": 0
}
```

这个旧版扁平 shape 仍可直接传给 `analyze quality --baseline`，便于已有项目删除 Node 检查脚本后
无缝迁移；重新执行 `--write-baseline` 会生成更严格的按 definition 格式。如果迁移把 `none` 改善为
`partial`，`typeNone` 会下降且 `typeNotFull` 不变；改善为 `full` 时二者都会下降。确有类型债务在
不同分类间迁移时，应在 PR 中解释并显式更新 baseline，而不是让一个总数相互抵消。baseline 归零后
保留 `analyze quality`，以阻止后续重新引入。

### 3.7 format 的边界

`calcit edit format` 负责可解析性、canonical serialization 和已知旧结构迁移；它不是完整的语义 linter。告警写到 stderr 且不会阻止格式化。CI 需要在 format 后检查 `git diff`，并单独读取 `check-types` / `weak-types --format json` 来执行项目自己的质量阈值。

### 3.8 Trait impl 从方法包迁移为 nominal impl

旧代码可能把 tag 作为 `defimpl` 的 trait 参数：

```cirru.no-check
defimpl :RenderImpl :Render $ .render
  fn (x) str x
```

这种写法继续参与普通 `.method` 分派，但现在明确视为**不具名的 inherent method bag**；它不会满足 `assert-traits`、函数/数据结构的 `:where` 约束，也不能被 `&trait-call` 选中。`calcit edit format` 会给出不阻断执行的 `W_LEGACY_INHERENT_IMPL` 迁移告警。需要能力约束的新代码应改成：

```cirru
let
    Render $ deftrait Render (.render :fn)
    RenderImpl $ defimpl RenderImpl Render
      .render $ fn (x) str x
  , RenderImpl
```

升级时还要修复以前被宽松实现接受的问题：

- concrete trait impl 必须完整实现声明的方法，且不能夹带 trait 未声明的方法；
- trait method 的值必须可调用；native 预处理在签名信息可用时还会检查函数签名；
- 同名方法不会跨 impl 拼接成一个 trait，也不会让两个独立 trait 互相冒充；
- list/map/set/string/number/struct/enum 等内建能力现在由 native 与 JS 共用的 nominal core impl 表提供。

建议在升级验证中显式覆盖两类负向用例：只有 `TraitA/.method` 时，`assert-traits value TraitB` 和 `&trait-call TraitB :method value` 都必须失败。

WASM 仍只是仓库内部验证后端，不承诺 trait runtime table。能在预处理阶段消除的 trait 元数据仍可参与编译；残留的 `&impl::new`、`impl-traits` 或 `&assert-traits` 会明确报出“不支持 runtime trait table”，而不是静默返回 `nil`。业务项目以 JS 为主，并用 native 执行宏和预处理。

---

## 4）Yarn Berry 升级检查

### 4.1 packageManager 固定

```json
{
  "packageManager": "yarn@4.12.0"
}
```

### 4.2 CI 基础模板（GitHub Actions）

```yaml
- uses: actions/setup-node@v6
  with:
    node-version: 24

- name: Enable Corepack
  run: |
    corepack enable
    corepack prepare yarn@4.12.0 --activate
    yarn --version

- uses: calcit-lang/setup-calcit@v1

- name: Install deps
  run: caps --ci && yarn install --immutable

- name: Validate Calcit snapshot and types
  run: |
    calcit calcit.cirru edit format
    git diff --exit-code -- calcit.cirru
    # `calcit config show` lists every configured :entries item, including default.
    while IFS= read -r entry; do
      if [ "$entry" = "default" ]; then
        calcit calcit.cirru --check-only
        calcit calcit.cirru --warn-dyn-method --check-only
      else
        calcit calcit.cirru --entry "$entry" --check-only
        calcit calcit.cirru --entry "$entry" --warn-dyn-method --check-only
      fi
    done < <(calcit calcit.cirru config show | awk '/^Snapshot Entries:/{in_entries=1; next} in_entries && /^  [^ ]/{print $1}')
    calcit calcit.cirru analyze quality --baseline config/calcit-quality.json

- name: Run project tests
  run: |
    calcit calcit.cirru test --tag unit --require-match --summary-only --format json
    calcit calcit.cirru --entry test
```

> ⚠️ CI 中安装的 Calcit 项目版本来自 `deps.cirru`，Action release 只决定安装器协议是否足够新。普通 workflow 不传 `version`；升级时修改 `deps.cirru`，并在需要新安装器能力时更新 `calcit-lang/setup-calcit`。不要在 `caps --ci` 之前运行 `calcit` 命令，否则会使用默认旧版模块缓存。完整模板见 [GitHub Actions](../installation/github-actions.md)。

说明：若项目依赖 `packageManager: "yarn@4.12.0"`，优先先执行 Corepack 激活，再让 CI 触发 Yarn。不要让 `setup-node` 的 Yarn cache 或其他 Yarn 调用早于 `corepack enable` / `corepack prepare`，否则可能误用 runner 上的全局 Yarn 1。 `caps --ci` 参数保证在 CI 加载模块时使用 HTTPS 协议，避免 CI 环境下的 SSH key 问题。

注意：`check-types`、`weak-types`、`deprecated` 仍是展示报告，不按命中数量失败；CI 使用
`analyze quality` 执行零目标或 baseline 策略。清零后则要求 unresolved dynamic、
unresolved/declared-optional nil debt 和 deprecated calls 均为 0。
`test --require-match` 会避免 tag 或 scope 写错后零测试仍退出成功。项目没有 named `test` entry 或
definition-attached unit tests 时，应删除对应示例行并替换成项目真实测试命令，而不是机械照抄。

### 4.3 lockfile 迁移

如果 `yarn install --immutable` 因 lockfile 格式变化失败：

1. 先执行一次 `yarn install` 生成新格式 lockfile；
2. 再执行 `yarn install --immutable` 做严格校验。

---

## 5）升级后最小验证矩阵

建议至少覆盖以下项目：

1. `calcit --version`
2. `caps upgrade --all`（确认无遗漏项或已按预期处理）
3. `caps tree`（确认根开发依赖存在，同时传递模块的开发依赖未进入图）
4. `yarn install --immutable`
5. `calcit calcit.cirru edit format` 后 `git diff --exit-code -- calcit.cirru`
6. default 与每个 named entry 的 `--check-only`
7. 每个 entry 的 `--warn-dyn-method --check-only`（普通检查全绿后启用）
8. 所有声明支持的 entry 行为测试（默认 once；watch 另行验收）
9. `analyze quality` 的 JSON baseline 或零目标（`check-types`、dynamic/nil `weak-types`、`deprecated` 仍作为定位报告）
10. `calcit test --require-match`、公开 namespace 的 `check-examples` 与 `docs check-md`
11. JS 项目的 codegen 加 Node/Vite 行为测试，而不只是生成成功
12. `package.json` 中与编译/构建相关的脚本
13. 类库项目在 Respo 等真实消费者中的回归证据

### 老项目失败时的定位顺序

| 阶段 | 命令 | 主要发现 | 是否自动阻断 |
| --- | --- | --- | --- |
| 依赖图 | `caps tree/status/verify` | 递归版本、链接、store/native 收据 | 状态异常会阻断；普通图告警用 `--strict` 收紧 |
| Snapshot 规范化 | `calcit edit format` + `git diff` | 旧 configs/schema 拼写和规范化建议 | format 告警不阻断，diff 需人工审阅 |
| entry 预处理 | `calcit --entry ... --check-only` | 配置、缺失定义、参数/返回值、数据与 trait 类型错误 | 错误或 warning 均阻断 |
| 动态分派 | `calcit --warn-dyn-method --check-only` | 动态 receiver、无法专门化的方法与未类型化 FFI 访问 | warning 会随 check-only 阻断 |
| 静态债务 | `analyze check-types/weak-types/deprecated --format json` | 覆盖率、dynamic、nil/Optional、废弃调用 | 报告本身不按命中数阻断，CI 比较 summary |
| 示例与测试 | `check-examples`、`docs check-md`、`calcit test --require-match` | API 示例、文档片段、definition-attached tests | 失败或未匹配测试时阻断 |
| 行为与后端 | entry、Node/Vite、项目测试 | native/JS/FFI 的真实行为差异 | 由进程退出码阻断 |

每次失败先查看终端诊断；需要完整运行栈时再看 `.calcit/error.cirru` 或执行
`calcit calcit.cirru query error`。针对单个定义可用 `calcit query context <ns/def> --format json`，针对
具体表达式可用诊断返回的 Snapshot path 调用 `calcit query type-at <ns/def> --path code@... --format json`。

call graph 的 `--show-unused` 只能作为 entry-relative 线索；公开 API 和替代入口可能被列为 unreachable，不能据此自动删除。
