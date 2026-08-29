---
title: "Calcit Agent 快速实践（局部查看与编辑优先）"
summary: "冷启动与高频闭环：Cirru/AST 必备规则、查询定位、结构化编辑、cursor 连续操作、验证与按需查文档"
scope: "core"
kind: "agent"
category: "run"
aliases:
  - "agent workflow"
  - "llm workflow"
  - "local editing guide"
  - "copilot workflow"
entry_for:
  - "calcit docs agents"
  - "calcit docs read agent-advanced.md"
id: core/agent
related:
  - core/docs/indexing
  - core/run/query
  - core/run/edit-tree
leads_to:
  - core/run/quick-start
---

# Calcit Agent 快速实践（局部查看与编辑优先）

本文是 Agent 每次进入 Calcit 项目时需要常驻上下文的最小操作契约。只保留高频规则和可执行闭环；低频命令、完整语法与复杂重构通过 `calcit docs` 按需读取。

## 0. 开始修改前

1. 先遵守当前仓库的 `AGENTS.md`、README 和用户要求；本文只补充 Calcit 源码操作规则。若仓库示例被当前 CLI 以 `Unrecognized argument` 拒绝，保持原约束意图，用该子命令的 live `--help` 换成当前参数，不要因此绕过 `calcit` 直接改 Snapshot。
2. 每个任务第一次执行 `calcit edit`、`calcit tree` 或 cursor mutation 前，必须先读取当前 CLI 内嵌的完整指南：

   ```bash
   calcit docs agents --full
   ```

3. `calcit.cirru` 是 **Cirru EDN 树形 Snapshot**，不是按行维护的文本源码。旧项目若只有 `compact.cirru`，先按 upgrade 指南迁移；当前工具会拒绝旧文件名。不要用 line patch、正则脚本或 formatter 直接改 Snapshot；使用 `calcit edit`、`calcit tree`、`calcit cursor`。
4. `CURSOR`、`FOLDED:*`、chunk 标题和 path annotation 都是展示信息，绝不能复制回 Snapshot。`cursor show --format json` 中 `tree` 才是真实节点，`preview_tree` 只是展示树。

会写回 Snapshot 的 `edit`、`tree`、`config` 和 cursor mutation 会先核对同目录
`deps.cirru :calcit-version`。若项目固定版本与当前 CLI 不一致，命令会在写入前失败；应改用项目固定的
Calcit 版本，或先显式执行 `caps deps.cirru upgrade --all` 升级项目。查询、dry-run、cursor 导航，以及尚未
声明 `:calcit-version` 的旧项目不受此门禁影响。

### 0.1 发现 Calcit 缺陷时：定位归属仓库并提交 Issue

发现 **Calcit 语言、编译器、运行时或 CLI 工具** 的可复现问题时，必须向该工具的维护仓库提交 GitHub Issue；不要把问题只留在当前项目的提交说明、错误 sidecar 或聊天记录里。发现 **类库/模块** 问题时，也必须提交到该类库的维护仓库，而不是误报到使用它的应用项目或 Calcit 核心仓库。

先收集最小复现、实际结果、预期结果、`calcit -v`、平台和相关命令。类库问题先从解析后的模块路径确认归属，再创建 Issue：

```bash
calcit query modules
# 从输出中复制该模块的实际目录；不要猜仓库名。
git -C '<module-directory>' remote get-url origin
# 将 origin 规范化为 OWNER/REPO 后，确认目标确实是对应 GitHub 仓库。
gh repo view OWNER/REPO --json nameWithOwner,url
mkdir -p .calcit/snippets
gh issue create --repo OWNER/REPO --title '<concise problem title>' --body-file .calcit/snippets/calcit-issue.md
```

`gh issue create --repo OWNER/REPO` 可显式提交到**非当前仓库**；不要依赖 cwd 或当前 Git remote 推断目标。若模块目录没有自己的 GitHub origin，先根据其路径、模块元数据、发布页或维护文档找出权威仓库；仍无法确认归属时，报告这一阻塞并向用户确认，不能把 Issue 猜测性地投到核心仓库。

Issue 正文至少包括：最小 Snapshot/snippet 或步骤、实际与预期行为、完整诊断输出、Calcit/模块版本、操作系统与架构。删除 token、私有路径、业务数据和其他机密；大型 Snapshot 应改为最小可公开复现。提交后记录并回报 Issue URL、`OWNER/REPO` 和所用版本，方便后续追踪。

以下命令默认在项目根目录读取 `calcit.cirru`。只有操作临时副本或非默认 Snapshot 时才显式写文件：

```bash
calcit query config
calcit .calcit/snippets/demo.cirru query config
```

当 cwd、`calcit.cirru` 或多个 Snapshot 可能混淆时，先选定文件，并在后续查询、mutation、验证中始终显式传同一个路径（如 `calcit ./calcit.cirru ...`）。遇到 `compact.cirru` 时暂停 mutation，先复制或重命名为 `calcit.cirru` 并按 upgrade 指南验证。非默认 Snapshot 会包含在 `Command:` 回显中；默认值为减少噪音会省略，需要审计展开后的全部默认选项时加 `--verbose`。必要时结合 `query config` 确认项目身份。

`calcit [snapshot-file]` 默认选择 `entries.default` 并按它的 `:mode`（`:native` / `:js`）单次运行；`--entry <name>` 选择其他入口。显式 `js` 保留为覆盖方式。只有明确需要监听时才加 `-w` / `--watch`。`calcit ir` 只用于编译器/生成结果调试，不作为日常构建或完成证明。这里的 snapshot 文件不要与 `--entry <named-entry>` 混淆。

## 1. 30 秒项目盘点

如果用户已经给出 `namespace/definition`，可直接从 `query context` 开始；否则依次执行：

```bash
calcit -v
calcit query config
calcit query ns
calcit query modules
# Dependency intent audit (when deps.cirru uses dev-dependencies):
caps tree
# Choose a module from the tree, then explain its root/transitive path:
caps why '<owner/repo>'
# 从 query ns 的输出中选择真实 namespace，再执行：
calcit query defs '<namespace>'
```

- `query config`：确认 init/reload、版本和项目配置。
- `query ns`：先发现 namespace，不要猜 `<ns>`。
- `query defs <ns>`：从真实定义名中选择 target。
- `query modules`：确认依赖边界；不要修改已安装依赖目录来代替当前项目修改。
- `caps tree` / `caps why`：审计 `deps.cirru` 的已解析下载图和传递来源；它们不分析
  Calcit 源码，因此不能单独证明模块是否被某个 entry、测试或 Markdown 示例实际使用。

### 1.1 依赖分组审计：runtime、development 与源码使用是三件事

`deps.cirru` 的 `:dependencies` 是消费者在运行或编译时需要的模块；
`:dev-dependencies` 仅供当前项目的测试、examples、Markdown 检查和维护任务使用。根项目会安装
两组，递归解析某个模块时只读取它自己的 `:dependencies`。先直接阅读 `deps.cirru` 的两个根分组，
再用下面流程审计；不要仅凭模块出现在 `.calcit/modules/` 就把它判为 runtime 依赖：

```bash
caps tree
caps why '<owner/repo>'
calcit calcit.cirru config modules
calcit calcit.cirru config modules --entry '<entry-name>'
calcit calcit.cirru --check-only
calcit calcit.cirru --entry '<entry-name>' --check-only
# Here --entry is the snapshot filename, not a named entry:
calcit calcit.cirru docs check-md README.md --entry calcit.cirru --failures-only
```

`caps tree` 和 `caps why` 回答“该仓库为什么被依赖解析器安装”；目前它们会合并显示两个根分组，
所以根归属仍以 `deps.cirru` 为准。`config modules` 回答“某个 entry 配置加载哪些模块”；每个 named
entry 都是独立配置，不能假设其模块继承 default。`--check-only` 只验证所选 entry 的可达预处理路径，
而 `docs check-md` 默认只带 default entry 的模块；有测试或文档专用模块时，须显式选择相应 entry 或
重复传入 `--dep`。动态加载、未调用的公开 API 和外部消费者不在这些静态结果的证明范围内。

注意：`config modules --entry` 与顶层 `--entry` 选择 named entry；`docs check-md --entry` 则选择用于
检查的 snapshot 文件（`calcit.cirru`），两者不是同一种参数。

读取源码优先使用 human/Cirru 输出；只有需要稳定字段、自动分支或静态证据时才使用 `--format json`。`--format json` 承诺 stdout 为单个 JSON envelope；某些命令的 `--json` 只是在人类输出后附加 JSON，具体以子命令 `--help` 为准。

## 2. 最小心智模型

| 概念       | 含义                                                         | 操作习惯                                      |
| ---------- | ------------------------------------------------------------ | --------------------------------------------- |
| Snapshot   | 整个项目的 EDN 数据树                                        | 只通过 `calcit` 修改                              |
| target     | `namespace/definition`                                       | 先从 `query ns/defs/find` 获取                |
| path       | definition 内的树坐标，如 `@3.2.1`                          | mutation 后可能变化，不长期缓存               |
| cursor     | 带 target、path 和 fingerprint 的本地选择                    | 连续编辑时优先使用，避免反复搬运数字坐标      |
| definition revision | definition 的内容版本，由 context/cursor 返回              | 判断语义证据或 cursor 是否过期                 |
| Snapshot revision | 整个 Snapshot 的内容版本，由 transaction dry-run 返回        | 传给 `--expect-revision` 阻止覆盖并发修改       |

`query def` 对大定义默认可能输出 chunked preview；先用 `query peek` 或默认 `query def` 看结构，确实需要完整定义时才用 `query def '<ns/def>' --raw`。不要把 `FOLDED:*` 或 chunk 标记当成源码。

path 使用从零开始的 child index：`@3.2` 表示先取 definition 根 list 的 child 3，再取其 child 2；空 path 表示 definition 根节点。结构 mutation 后旧 path 可能失效，优先重新查询或使用 cursor。必须直接使用旧数字 path 时，`tree replace/delete/insert-*` 推荐同时传 `--expect 'quote ...'`；实际节点或插入锚点不匹配时命令会在写入前失败。

搜索选择规则：

- 按定义名跨 namespace 找：`calcit query find <symbol>`。
- 在源码 leaf 中找字符串、symbol、tag：`calcit query search <leaf> --filter '<ns/def>'`。
- 按一段树形表达式找：`calcit query search-expr '<cirru-expr>' --filter '<ns/def>'`。

编辑选择规则：

- 新增/移动 definition，修改 namespace、import、schema、examples：`calcit edit`。
- 一次局部节点修改：`calcit tree`，优先 `search-replace`，其次明确 path 的操作。
- 在一个复杂表达式中连续移动和修改：`calcit cursor` 与 `@cursor`。
- 多个 mutation 必须一起成功：`calcit edit transaction`，先 `--dry-run`；主格式是 Cirru EDN，先运行 `calcit docs read edit-tree.md 'Atomic Transactions'` 查看最小 operation 文件和 revision 提交流程。

同一个 Snapshot 的写命令必须串行执行，包括 `config`、`edit`、`tree` 和 cursor mutation；两个进程同时读取再保存会发生最后写入覆盖。需要并行时使用独立 Snapshot/worktree，需要同一文件内的原子多步修改时使用 transaction 和 `--expect-revision`。

### Feature-level architecture scaffold

当任务包含多个相互调用的 definition 时，先把架构写入版本控制中的
`docs/architectures/<feature>.cirru`，再运行 scaffold planner：

```bash
calcit calcit.cirru edit scaffold --file docs/architectures/order.cirru \
  --dry-run --format edn
```

Architecture plan 是 Cirru EDN 数据：definition FQN 和 `:params` 使用
Symbol，调用/类型关系使用 `:: :call` / `:: :type` anonymous enum，不能用
混合类型的 `[]` 表示。先查看 `reconciliation`、warnings 与 work items；
已有 definition 仍会出现在结果中。只有确认 revision 后才 apply：

```bash
calcit calcit.cirru edit scaffold --file docs/architectures/order.cirru \
  --expect-revision md5:... --format edn
```

apply 只创建缺失的 `:ensure` definition，不覆盖已有 code/doc/schema。新建
函数带 `:scaffold` tag 和 `todo!`，因此会产生 `W_TODO`；它们是待分配的
work items，不是已经完成的实现。父 Agent 可以并行分发 work item，但第一
版仍应在最新 Snapshot 上串行合并实现，使用 revision/write-set 防止陈旧写入。

Architecture 计划放在 `docs/`，而 `.calcit/` 只保存本机 cursor、error 和
snippets 等临时状态；不要把需要评审的设计落进隐藏目录。

## 3. 高频黄金路径：查询 → 编辑 → 验证

下面是需要替换 `<...>` 占位符的任务模板，不能原样执行。target、needle 和 replacement 必须来自当前项目及用户目标。先看搜索结果中的 `[#N]`，确认后再用同一序号设置 cursor：

```bash
calcit query context '<namespace/definition>' --format json
calcit query search '<existing-leaf>' --filter '<namespace/definition>' --exact
calcit query search '<existing-leaf>' --filter '<namespace/definition>' --exact --set-cursor 0
calcit cursor show
calcit cursor apply replace --code 'quote <replacement-leaf>'
calcit tree show @cursor --path @cursor
calcit query type-at @cursor --path @cursor --format json
calcit analyze check-examples --ns '<namespace>' --def '<definition>'
calcit test '<namespace>/<definition>'
```

`type-at --format json` 的语义路径可能是 `code@3.2`，而 `tree --path` 需要 `@3.2`；不要把仍含 `code@` 的 follow-up 命令直接交给 `tree`。

最后运行当前仓库规定的测试和目标 codegen。只有项目目标是 JS 时，`calcit js` 才是对应的编译检查；它不是所有 Calcit 项目的通用完成证明。

### 废弃 API 清理

迁移 API 时，先运行 `calcit analyze deprecated --ns-prefix <package>` 查看调用路径和替换说明；清零前保留兼容 API 及其 `:deprecated` tag。CI 或迁移 gate 使用 `calcit analyze deprecated --ns-prefix <package> --summary-only --format json`，仅当目标范围 `calls` 为 `0` 时再删除旧 API。

对于唯一 leaf 的小改动，可以不用 cursor：

```bash
calcit query search '|Old title' --filter 'app.main/comp-page' --exact
calcit tree search-replace 'app.main/comp-page' \
  --pattern '|Old title' --code 'quote "|New title"'
calcit query search '|New title' --filter 'app.main/comp-page' --exact
```

多匹配时 `search-replace` 会拒绝猜测；查看候选后用 `--pick <N>`，或改用 search → cursor → apply。

`--set-cursor` 会选中匹配 leaf。若要在它所在的表达式旁插入 sibling，先移动到 parent；插入后 cursor 仍跟随原表达式，再用 `next` 选中新节点：

```bash
calcit query search '<leaf-in-expression>' --filter '<namespace/definition>' --exact --set-cursor 0
calcit cursor parent
calcit cursor apply insert-after --code 'quote $ <new-expression>'
calcit --cursor-after focus cursor next
```

新增 definition 与 import 的最短路径：

```bash
calcit edit add-ns app.util
calcit edit def 'app.util/double' \
  --code 'quote $ defn double (x) (* x 2)'
calcit edit add-import app.main \
  --code 'quote $ app.util :refer $ double'
calcit query def 'app.util/double'
calcit --check-only
```

`edit add-import` 接收一条 import rule body，不包含 `:require`；优先使用它。只有明确要整体替换全部 imports 时才使用 `edit imports`。已有 definition 或同来源 import 需要覆盖时必须显式加 `--overwrite`。优先局部 tree mutation，不要为了改几个节点整段覆盖。最后仍需运行项目规定的测试与 codegen。

整体替换多条 import 时，`edit imports --file imports.cirru` 的主格式是 quoted Cirru AST，而不是 JSON：

```cirru
quote $ []
  respo.core :refer $ div span
  respo-ui.core :as ui
```

CLI 去掉外层 `quote`，确认 `[]` marker 后，把数组内部每个表达式作为一条 import rule；不要包含外层 `:require`。JSON 数组仅作为互操作兼容格式保留。

## 4. Cursor 连续编辑

最常用的 cursor 循环只有四步：搜索选中、展示、修改、再展示。

```bash
calcit query search render-item --filter 'app.main/render!' --exact
calcit query search render-item --filter 'app.main/render!' --exact --set-cursor 0
calcit cursor show
calcit cursor apply wrap --code 'quote $ when visible? self'
calcit cursor show
```

常用补充：

- `cursor parent`、`cursor child [index]` / `child --last`：进入父子层级。
- `cursor next/prev --count N`：跨多个 sibling；跨 list 边界使用 `forward/backward --count N`。
- `cursor duplicate --at before|after`：复制选中表达式并选中新副本，不覆盖 clipboard。
- `cursor cut` 后选中 parent；`cursor paste` 后选中新节点，`--at` 支持 `before|after|prepend-child|append-child|replace`。
- `query search ... --start-path @cursor --set-cursor N`：只在当前 subtree 中继续搜索。
- `query next/prev`：重新计算上次通过 `--set-cursor` 保存的搜索并跳到相邻结果，不保存完整结果列表；Snapshot 已变化时先重跑原查询显式选中。
- `cursor anchor` → 移动 → `cursor region`：确认同一 parent 下的连续 sibling 范围；结束后 `clear-anchor`。
- `cursor mark <name>` / `goto <name>`：保存和恢复最多 16 个高频位置；短期绕行仍优先使用 `push/pop`。
- `query context @cursor`、`tree show @cursor --path @cursor`：后续命令不再重复 target/path。
- `cursor back` 只回退 cursor 位置，**不会撤销源码修改**。

cursor 密集操作可把顶层选项写在命令前，如 `calcit --cursor-after focus cursor forward --count 4`，让每次移动立即展示上下文。需要机器确认真实选中节点时，使用 `calcit cursor show --format json --view node`。

`.calcit/` 是项目本地状态目录，应整体加入 `.gitignore`。其中 `cursor.cirru` 保存 active cursor、单一 anchor、最多 16 个 marks、last query、有限 history/stack 与 clipboard，硬上限 64 KiB；`error.cirru` 保存最近一次持久化 runtime/watcher stack，多行输入临时片段可放 `snippets/`。进入复制项目或已有 worktree 时先 `cursor show`；若选择与当前任务无关，执行 `cursor clear`。目前只有一个 active cursor，不负责多个进程的并发写入；并行 Agent 使用独立 worktree/Snapshot。

非法 cursor navigation 会保持 Snapshot 和 cursor 不变，失败后用 `cursor show` 确认。`unwrap` 会把所选 list 的所有 children splice 到 parent；对含额外语法的 wrapper，它不是 `wrap` 的撤销操作。

Paredit 的 slurp/barf、history/stack、结构化 clipboard、stale relocation 等细节按需查询：

```bash
calcit docs read edit-tree.md 'Persistent Tree Cursor'
calcit cursor --help
```

## 5. Cirru / Calcit 语法生存指南

先区分三个阶段：**Cirru 文本 → list/leaf AST → Calcit 求值**。`calcit.cirru` 再用 Cirru EDN 保存整个 Snapshot，其中 definition 的 `:code` 是 quoted AST。`calcit cirru parse` 只证明文本能解析并展示树形；它不保证这棵树具有预期的 Calcit 语义。`calcit cirru parse-edn` 查看的是 EDN 数据，不是同一个解析阶段。

### 5.1 先看 AST，不要按传统 Lisp 猜括号

Cirru 用 2 个空格表示一层嵌套，不使用 tab；同一行的空格分隔节点，圆括号创建行内 child list，`$` 把其右侧整体折叠成一个 child，连续 `$` 从右向左结合：

```text
a b c          => ["a", "b", "c"]
a $ b c        => ["a", ["b", "c"]]
a (b c) d      => ["a", ["b", "c"], "d"]
a $ b $ c d    => ["a", ["b", ["c", "d"]]]
```

下面的多行形式同样得到 `a (b c) d`：

```cirru.no-check
a
  b c
  , d
```

缩进新行本身会形成 list，所以不带逗号的裸 `d` 是 `["d"]`，Calcit 会把它当作零参数调用；`, d` 才把 leaf `d` 放进父表达式。这个规则尤其影响函数、`let` 和分支的最后一个返回值：

```cirru.no-check
fn (x)
  , x
```

圆括号不是可随意增加的装饰。`range 3` 是 `["range", "3"]`，而顶层 `(range 3)` 是 `[["range", "3"]]`，会尝试把内层结果再次当作函数调用。类似地，`x $ (f a)` 会产生 `["x", [["f", "a"]]]`；通常应写 `x $ f a` 或 `x (f a)`。

在 Respo 属性 map 中，缩进同样决定 key 属于哪一层；下面的 `:click` 属于 `:on` 对应的内层 `{}`：

```cirru.no-check
div
  {}
    :class-name $ str-spaced css/a css/b
    :on $ {}
      :click $ fn (e d!)
        js/log e
```

### 5.2 高频字面量与求值陷阱

- `hello` 是 symbol，`|hello` 是 string，`:hello` 是 tag；字符串必须保留 `|`。含空格写 `"|hello world"`，其中双引号只保护一个含空格 token，单独写 `"hello"` 仍是 symbol。
- `[]`、`{}`、`#{}` 是 Calcit 的集合构造符号，不是包围源码的定界符。作为普通参数的裸 `[]` 是函数值，不是空 list；可靠写法是先 `init $ []` 再传 `init`，或显式使用 `([])`。
- `let` bindings 必须是 pair list：单行写 `let ((x 1)) ...`；多行时 bindings 比 body 多缩进一层：

  ```cirru.no-check
  let
      x 1
      y $ + x 1
    + x y
  ```

- `map`、`filter`、`foldl` 等 Calcit 集合函数把集合放在前面；不确定参数顺序时运行 `calcit query examples calcit.core/map` 或查询对应定义，以当前签名和示例为准。
- 改动缩进、`$`、`,` 或圆括号都会改变 AST 和 path；修改后重新 show/search，或继续使用 cursor。

### 5.3 可选参数优先使用 `Option`

新代码优先用 `Option<T>` 表达“可能没有值”，减少通过参数列表里的 `?` 设置可选参数，也减少用 `nil` 表达缺失。函数末尾连续声明为 `Option<T>` 的参数可以在调用时省略；Calcit 会依次补成 `%none`。同样，已知 Struct 定义可以直接用 `Struct :field value` 构造，声明为 `Option<T>` 的字段可以省略并自动得到 `%none`：

```cirru.no-check
defn request (url trace-id timeout-ms)
  hint-fn $ {}
    :args $ [] 'String (:: 'Option 'String) (:: 'Option 'Number)
    :return 'Unit
  println url trace-id timeout-ms

request |/health
request |/health (%some |trace-1)

defstruct Profile (:name 'String) (:bio (:: 'Option 'String))
Profile :name |Ada
```

这项语法糖只处理**结尾连续的** `Option` 参数：位于必填参数之前的 `Option` 仍然必须显式传 `(%none)` 或 `(%some value)`，带 rest 参数的函数也不会自动补值。`?` 参数仍用于兼容已有的非类型化 API，其缺省值是 `nil`；修改旧接口时，优先逐步迁移到 `Option`。在 FFI 或非类型化边界之外，缺失值使用 `Option`，失败使用 `Result`，无有效返回值使用 `Unit`。

Option/Result 的级联优先使用接收者方法，不要在每一层都 `unwrap`：

```cirru.no-check
profile .and-then
  fn (profile)
    (get-in profile $ [] :account) .and-then
      fn (account) $ get account :name

parsed .and-then
  fn (value) $ validate value
```

连续的 Option 步骤可以实验性使用 `option:let`。Result 流程直接使用接收者
`.and-then`，让错误类型转换保持可见：

```cirru.no-check
let
    source $ fs:path |data.cirru
    content-result source.read-text
  content-result.and-then $ fn (content)
    (parse-data content) .and-then $ fn (data)
      save-data data
```

`fs:path` 把 UTF-8 字符串显式构造成 `FsPath`，不执行规范化或文件系统访问。
`FsPath` 上的 `.read-text`、`.read-dir`、`.walk-dir` 与 `.write-text` 返回
`Result<...,String>`；String 不提供文件效果方法。`try-read-file`、`try-read-dir`、
`try-write-file` 以及底层 raising procedures 仍作为兼容入口。
这些文件效果支持 native 与生成的 JavaScript；WASM 尚未提供宿主文件效果。

`option:let` 使用普通 `let` 的 binding pair 结构。每个右侧和最终 body 都必须保持
Option 容器；Result 错误类型需要转换时显式使用 `.map-err`。

需要尝试备用来源时使用 `.or-else`；它只在 `none`/`err` 分支调用 fallback。`.unwrap` 只适合已经由 `tag-match`、`.some?` 或明确不变量证明为 `some` 的位置；默认值用 `.unwrap-or`，继续转换用 `.map` / `.and-then`。接收者已静态推断为 `Option`/`Result` 时，避免使用 `option:*` / `result:*` 的函数形式，以便接收者类型和类型流保持可见；未类型化 legacy 数据或 core 边界才保留直接 helper。

`get-in` 返回 `Option<T>`，适合 Map/List/字符串等可能缺失的路径；路径进入 Struct 时应改用类型化的 `(:field value)` 访问，字段需要可缺失时在 Struct 中声明 `Option<T>`。`update-in` 的 updater 接收 `Option<T>`，缺失分支应显式处理，不要无条件 unwrap：

```cirru.no-check
update-in data ([] :profile :visits)
  fn (current)
    current .unwrap-or 1
```

这样可以让缺失、默认值和失败在类型上分开，而不是继续依赖 `nil` 或组件临时的 `read-field` 函数。

已知具名 Struct 的字段读取一律写成 `(:field value)`（接收者优先的 invoke 简写为 `value.:field`），不要在应用代码中生成 `&struct:get`。检查器会验证字段、返回声明类型，并把读取自动降为 `&struct:nth value <index>`；直接写低层 primitive 会隐藏源码中的类型意图，并触发 `W_STRUCT_RAW_ACCESS`。如果 `(:field value)` 报 `W_REQUIRED_STRUCT_FIELD_TYPE`，应补全 schema、先 narrow/unwrap，不能改写成 `&struct:get` 绕过分析。若诊断中的接收者只剩未限定的 `'Router` 一类 nominal TypeRef，应恢复或显式写成 `'app.schema/Router`，并检查声明所在依赖是否正确加载；嵌套字段声明中的同 namespace 短类型会由检查器自动保留声明 namespace。只有可复用 `defimpl` 尚未绑定具体 Struct 的实现体以及 core/runtime 底层代码，才把 `&struct:get` 作为明确的动态边界。`W_STRUCT_*` 属于项目源码迁移提示，不会要求调用方修改已安装依赖中的旧实现。

### 5.4 CLI 的 `quote` 是代码/数据边界

所有接收 AST 的 Cirru 文本输入都必须让 `quote` **恰好包住一个节点**；提交 mutation 时 CLI 会剥离这个 transport wrapper。JSON 数组 AST 是兼容输入，不需要 `quote`，但不要手写大型 JSON AST。

```text
symbol leaf:    quote new-name
string leaf:    quote |hello
spaced string:  quote "|hello world"
tag / number:   quote :ready    /    quote 1
expression:     quote $ println |hello
empty list/map: quote $ []      /    quote $ {}
```

不要写 `quote println |hello`：它给 `quote` 两个 payload。表达式应写成 `quote $ println |hello` 或 `quote (println |hello)`。

| 输入方式              | 适用场景                             |
| --------------------- | ------------------------------------ |
| `--code 'quote ...'`  | 简短单行输入                         |
| `--file <file>`       | 需要复用、审阅或 transaction 的输入；临时文件放 `.calcit/snippets/` |
| 省略两者，从 stdin 读 | 一次性多行内容，避免 Shell 转义      |

修改命令没有 `--stdin` 参数。多行内容直接省略 `--file/--code`：

```bash
calcit tree replace 'app.main/main!' --path '@3.1' <<'END'
quote $ if ready?
  render-ready
  render-loading
END
```

不要把仓库相关的 snippet 放进全局 `/tmp`；它脱离项目生命周期，也容易被 Agent 在后续命令中误用。需要落盘时先 `mkdir -p .calcit/snippets`，并确认 `.calcit/` 已加入项目 `.gitignore`。CLI 收到 `/tmp/...` 或 `/private/tmp/...` 的 Snapshot/`--file` 路径时会在 stderr 给出这一迁移提示，不污染 JSON stdout。

### 5.5 写入前先解析，写入后再做语义检查

```bash
calcit cirru parse -e --validate 'a $ b c'
calcit cirru parse --validate 'fn (x)
  , x'
calcit cirru show-guide
```

`-e` 适合单行 expression 文本；多行 block 即使只表示一个 AST expression，也省略 `-e`。检查 JSON 形状是否符合预期后，再用 `calcit eval`、`calcit --check-only` 或项目测试验证 Calcit 语义。Shell 中 `$`、反引号、`|`、`>` 有特殊含义；短输入用单引号，多行 mutation 用 heredoc。

## 6. 验证与失败恢复

按从便宜到昂贵的顺序形成闭环：

1. 结构：`cursor show` 或 `tree show`，确认实际 subtree。
2. 语义：`query type-at ... --format json`，运行 `analyze check-types --summary-only`；发布审计加 `--deps --format json`，以 Snapshot loader 成功解析作为 strict macro schema 的门槛。旧的 runtime `Fn` / `Dynamic` macro schema 必须用最终兼容版本 0.13.51 迁移为显式 Macro contract，不要自动猜测 syntax/expansion 类型。看到 `W_TYPE_COVERAGE_GAPS` 后继续执行 `analyze weak-types --only schema-dynamic,unresolved-type-slot,code-dynamic --intent unresolved --summary-only`，再对命中范围去掉 `--summary-only` 查看 path、impact 与 suggestion。项目门禁使用 `analyze quality`（零容忍）；存量债务先执行一次 `analyze quality --write-baseline config/calcit-quality.json`，人工审阅后把同一文件提交到仓库，CI 改执行 `analyze quality --baseline config/calcit-quality.json`。不要再拼接多个 JSON 后交给 JavaScript 比较。门禁失败时保留 `--format json` 的单一 envelope，并按其中的 definition/path 回到 `weak-types` 或 `query type-at` 定位。
3. definition：`analyze check-examples --ns <ns> --def <def>`；存在 definition-attached tests 时运行 `calcit test <ns>/<def>`。
4. 项目：运行 `calcit test` 及仓库规定的 entry 和测试；默认 `calcit test` 只发现当前输入 snapshot 定义的命名空间，不触发 `calcit-core.cirru` 或外部模块中的测试。变更范围明确时可先用 `calcit test --affected <ns>/<def>` 做静态依赖筛选，但提交前仍按仓库要求执行全量门禁。CI/Agent 按 tag 或 affected 筛选时加 `--require-match`，避免空选择误报成功；大套件可加 `--summary-only --format json` 保持 stdout 紧凑可解析。只有项目目标是 JavaScript 时才运行对应的 `calcit js` codegen。

`type-at` 的 unresolved/dynamic warning 只表示静态证据不足；`check-examples` 输出 `No functions with examples` 且退出 0 只表示没有 example 覆盖。二者都不是完成证明，仍要继续项目级 check、测试和目标 codegen。

`calcit query tests <ns>/<def>` 查询 definition-attached tests；`calcit edit add-test <ns>/<def> <name> --code 'quote $ ...'` 添加稳定命名的测试，`calcit edit rm-test <ns>/<def> <name>` 按名称删除。`calcit test --affected <ns>/<def>` 使用编译后的传递依赖图选择测试；静态分析失败的测试会保守地被选中并报告为失败，不会静默漏测。

不要用多个 `'Dynamic` 假装多态：参数与返回共享类型时声明 `:generics`/TypeVar，只依赖能力时增加 trait `:where`，同质 collection/ref 保留 type arg，有限异构值使用 enum。类型写法统一用 quoted symbols，例如 `'String`、`'Number`、`'List` 和 `'Dynamic`；`:any`、`:dynamic` 等旧 tag 写法仅为兼容输入，运行 `calcit edit format` 后会在类型位置规范化。只有明确的 FFI、global state 或 macro 边界保留 dynamic，并尽快在进入 typed code 时 validate/convert。

每次 `calcit` 执行或编译都会在 stderr 审计项目自身的 Dynamic 类型位置：少量仅提示，达到较高占比会告警。该提示不写入 stdout，也不替代 `analyze check-types` / `analyze weak-types`；Agent 应先查看告警，再用 `calcit analyze weak-types --intent unresolved --format json` 定位并逐步收窄类型。

`:: :tag ...` 是匿名 Enum 字面量；当已有 Enum 定义在头部时，直接使用 `Enum :tag ...`，类型分析会检查变体和 payload，并在预处理阶段降为命名构造。只有需要显式携带运行时 enum prototype、跨模块动态构造或兼容旧代码时才使用 `%:: Enum :tag ...`。不要为了绕过类型检查而主动选择 `%::`。

| 现象                   | 恢复动作                                                                 |
| ---------------------- | ------------------------------------------------------------------------ |
| `Invalid path`         | 重新 `query search`，或用 `--set-cursor`；不要继续复用旧坐标             |
| cursor stale/ambiguous | `calcit cursor show` 检查；无法唯一恢复时重新 search + set                   |
| 输入解析失败           | `calcit cirru parse -e` 预检；复杂输入改用 heredoc/文件                      |
| parse/preprocess/query/edit 失败 | 先以当前命令 stderr 为准；这些失败不保证刷新错误 sidecar                 |
| runtime/watcher stack  | `calcit query error` 读取最近持久化栈；若提示 stale，不要追查其中的旧错误      |
| 多步修改范围不确定     | transaction `--dry-run --format json`，再带 `--expect-revision` 提交     |

优先用 `--set-cursor` 避免手工转换语义路径与 tree path。

## 7. 不知道时，通过 CLI 渐进查询

不要在本页复制完整能力地图，也不要凭记忆猜低频参数。先看 live help，再从 scope → 文件 → section 逐层读取：

```bash
calcit --help
calcit tree --help
calcit tree replace --help

calcit docs search 'cursor'
calcit docs sections edit-tree.md
calcit docs read edit-tree.md 'Persistent Tree Cursor'
calcit docs read-lines agent-advanced.md --start 1 --lines 80
```

常见主题路由：

| 主题                         | 查询入口                                                        |
| ---------------------------- | --------------------------------------------------------------- |
| Cirru 语法、AST 与常见误写   | `calcit cirru show-guide`；`calcit docs read cirru-syntax.md 'Common Mistakes'` |
| Cirru EDN 数据层             | `calcit cirru parse-edn --help`；`calcit docs read edn.md --full`       |
| 历史影响与迁移说明          | `calcit docs read from-clojure.md --full`                              |
| tree/cursor/transaction      | `calcit docs read edit-tree.md --full`                              |
| query/context/type-at        | `calcit docs read query.md --full`                                  |
| 复杂重构与历史陷阱           | `calcit docs read agent-advanced.md --full`                         |
| Snapshot、deps 与项目结构    | `calcit docs read project-structure.md --full`                      |
| 类型覆盖与 dynamic 审计      | `calcit docs read static-analysis.md --full`；`calcit analyze --help`   |
| 类库发布前验收与质量门禁     | `calcit docs read library-quality.md --full`                       |
| run/watch/JS codegen         | `calcit docs read cli-options.md 'Common Usage Patterns'`           |
| 错误排查                     | `calcit docs read debugging.md --full`；`calcit query error`            |
| 文档图与 frontmatter         | `calcit docs read docs-indexing.md --full`；`calcit docs graph --help`  |
| 安装模块的 API/示例          | `calcit docs scopes` → `calcit docs search <kw> --module <module>`      |

定义级资料优先从源码元数据查询：`calcit query schema '<ns/def>'`、`calcit query examples '<ns/def>'`、`calcit query usages '<ns/def>'`。远程库发现、program diff、call graph、JS escape 等低频能力直接从对应 `--help` 开始。

## 8. 完成检查

- target 来自查询结果，不是猜测。
- 修改前看过目标 subtree，修改后重新 show/search。
- 没有把 `CURSOR`、`FOLDED:*`、chunk 或 path annotation 写回源码。
- 小改动没有整段 overwrite；多步原子修改使用 revision precondition。
- 运行了与改动范围和项目 target 匹配的验证。
- 没有修改依赖缓存、生成目录或无关文件。
