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
  - "cr docs agents"
  - "cr docs read agent-advanced.md"
id: core/agent
related:
  - core/docs/indexing
  - core/run/query
  - core/run/edit-tree
leads_to:
  - core/run/quick-start
---

# Calcit Agent 快速实践（局部查看与编辑优先）

本文是 Agent 每次进入 Calcit 项目时需要常驻上下文的最小操作契约。只保留高频规则和可执行闭环；低频命令、完整语法与复杂重构通过 `cr docs` 按需读取。

## 0. 开始修改前

1. 先遵守当前仓库的 `AGENTS.md`、README 和用户要求；本文只补充 Calcit 源码操作规则。若仓库示例被当前 CLI 以 `Unrecognized argument` 拒绝，保持原约束意图，用该子命令的 live `--help` 换成当前参数，不要因此绕过 `cr` 直接改 Snapshot。
2. 每个任务第一次执行 `cr edit`、`cr tree` 或 cursor mutation 前，必须先读取当前 CLI 内嵌的完整指南：

   ```bash
   cr docs agents --full
   ```

3. `calcit.cirru`（旧项目可能是 `compact.cirru`）是 **Cirru EDN 树形 Snapshot**，不是按行维护的文本源码。不要用 line patch、正则脚本或 formatter 直接改它；使用 `cr edit`、`cr tree`、`cr cursor`。
4. `CURSOR`、`FOLDED:*`、chunk 标题和 path annotation 都是展示信息，绝不能复制回 Snapshot。`cursor show --format json` 中 `tree` 才是真实节点，`preview_tree` 只是展示树。

### 0.1 发现 Calcit 缺陷时：定位归属仓库并提交 Issue

发现 **Calcit 语言、编译器、运行时或 CLI 工具** 的可复现问题时，必须向该工具的维护仓库提交 GitHub Issue；不要把问题只留在当前项目的提交说明、错误 sidecar 或聊天记录里。发现 **类库/模块** 问题时，也必须提交到该类库的维护仓库，而不是误报到使用它的应用项目或 Calcit 核心仓库。

先收集最小复现、实际结果、预期结果、`cr -v`、平台和相关命令。类库问题先从解析后的模块路径确认归属，再创建 Issue：

```bash
cr query modules
# 从输出中复制该模块的实际目录；不要猜仓库名。
git -C '<module-directory>' remote get-url origin
# 将 origin 规范化为 OWNER/REPO 后，确认目标确实是对应 GitHub 仓库。
gh repo view OWNER/REPO --json nameWithOwner,url
gh issue create --repo OWNER/REPO --title '<concise problem title>' --body-file /tmp/calcit-issue.md
```

`gh issue create --repo OWNER/REPO` 可显式提交到**非当前仓库**；不要依赖 cwd 或当前 Git remote 推断目标。若模块目录没有自己的 GitHub origin，先根据其路径、模块元数据、发布页或维护文档找出权威仓库；仍无法确认归属时，报告这一阻塞并向用户确认，不能把 Issue 猜测性地投到核心仓库。

Issue 正文至少包括：最小 Snapshot/snippet 或步骤、实际与预期行为、完整诊断输出、Calcit/模块版本、操作系统与架构。删除 token、私有路径、业务数据和其他机密；大型 Snapshot 应改为最小可公开复现。提交后记录并回报 Issue URL、`OWNER/REPO` 和所用版本，方便后续追踪。

以下命令默认在项目根目录读取 `calcit.cirru`。只有操作临时副本或非默认 Snapshot 时才显式写文件：

```bash
cr query config
cr /tmp/demo.cirru query config
```

当 cwd、`calcit.cirru` / `compact.cirru` 或多个 Snapshot 可能混淆时，先选定文件，并在后续查询、mutation、验证中始终显式传同一个路径（如 `cr ./calcit.cirru ...`）。`Command:` 回显可能省略或归一化 input，不能用它证明实际文件身份。

`cr [snapshot-file]` 默认选择 `entries.default` 并按它的 `:mode`（`:native` / `:js`）单次运行；`--entry <name>` 选择其他入口。显式 `js` 保留为覆盖方式。只有明确需要监听时才加 `-w` / `--watch`。`cr ir` 只用于编译器/生成结果调试，不作为日常构建或完成证明。这里的 snapshot 文件不要与 `--entry <named-entry>` 混淆。

## 1. 30 秒项目盘点

如果用户已经给出 `namespace/definition`，可直接从 `query context` 开始；否则依次执行：

```bash
cr -v
cr query config
cr query ns
cr query modules
# 从 query ns 的输出中选择真实 namespace，再执行：
cr query defs '<namespace>'
```

- `query config`：确认 init/reload、版本和项目配置。
- `query ns`：先发现 namespace，不要猜 `<ns>`。
- `query defs <ns>`：从真实定义名中选择 target。
- `query modules`：确认依赖边界；不要修改已安装依赖目录来代替当前项目修改。

读取源码优先使用 human/Cirru 输出；只有需要稳定字段、自动分支或静态证据时才使用 `--format json`。`--format json` 承诺 stdout 为单个 JSON envelope；某些命令的 `--json` 只是在人类输出后附加 JSON，具体以子命令 `--help` 为准。

## 2. 最小心智模型

| 概念       | 含义                                                         | 操作习惯                                      |
| ---------- | ------------------------------------------------------------ | --------------------------------------------- |
| Snapshot   | 整个项目的 EDN 数据树                                        | 只通过 `cr` 修改                              |
| target     | `namespace/definition`                                       | 先从 `query ns/defs/find` 获取                |
| path       | definition 内的树坐标，如 `@3.2.1`                          | mutation 后可能变化，不长期缓存               |
| cursor     | 带 target、path 和 fingerprint 的本地选择                    | 连续编辑时优先使用，避免反复搬运数字坐标      |
| definition revision | definition 的内容版本，由 context/cursor 返回              | 判断语义证据或 cursor 是否过期                 |
| Snapshot revision | 整个 Snapshot 的内容版本，由 transaction dry-run 返回        | 传给 `--expect-revision` 阻止覆盖并发修改       |

`query def` 对大定义默认可能输出 chunked preview；先用 `query peek` 或默认 `query def` 看结构，确实需要完整定义时才用 `query def '<ns/def>' --raw`。不要把 `FOLDED:*` 或 chunk 标记当成源码。

path 使用从零开始的 child index：`@3.2` 表示先取 definition 根 list 的 child 3，再取其 child 2；空 path 表示 definition 根节点。结构 mutation 后旧 path 可能失效，优先重新查询或使用 cursor。

搜索选择规则：

- 按定义名跨 namespace 找：`cr query find <symbol>`。
- 在源码 leaf 中找字符串、symbol、tag：`cr query search <leaf> --filter '<ns/def>'`。
- 按一段树形表达式找：`cr query search-expr '<cirru-expr>' --filter '<ns/def>'`。

编辑选择规则：

- 新增/移动 definition，修改 namespace、import、schema、examples：`cr edit`。
- 一次局部节点修改：`cr tree`，优先 `search-replace`，其次明确 path 的操作。
- 在一个复杂表达式中连续移动和修改：`cr cursor` 与 `@cursor`。
- 多个 mutation 必须一起成功：`cr edit transaction`，先 `--dry-run`；主格式是 Cirru EDN，先运行 `cr docs read edit-tree.md 'Atomic Transactions'` 查看最小 operation 文件和 revision 提交流程。

## 3. 高频黄金路径：查询 → 编辑 → 验证

下面是需要替换 `<...>` 占位符的任务模板，不能原样执行。target、needle 和 replacement 必须来自当前项目及用户目标。先看搜索结果中的 `[#N]`，确认后再用同一序号设置 cursor：

```bash
cr query context '<namespace/definition>' --format json
cr query search '<existing-leaf>' --filter '<namespace/definition>' --exact
cr query search '<existing-leaf>' --filter '<namespace/definition>' --exact --set-cursor 0
cr cursor show
cr cursor apply replace --code 'quote <replacement-leaf>'
cr tree show @cursor --path @cursor
cr query type-at @cursor --path @cursor --format json
cr analyze check-examples --ns '<namespace>' --def '<definition>'
```

`type-at --format json` 的语义路径可能是 `code@3.2`，而 `tree --path` 需要 `@3.2`；不要把仍含 `code@` 的 follow-up 命令直接交给 `tree`。

最后运行当前仓库规定的测试和目标 codegen。只有项目目标是 JS 时，`cr js` 才是对应的编译检查；它不是所有 Calcit 项目的通用完成证明。

对于唯一 leaf 的小改动，可以不用 cursor：

```bash
cr query search '|Old title' --filter 'app.main/comp-page' --exact
cr tree search-replace 'app.main/comp-page' \
  --pattern '|Old title' --code 'quote "|New title"'
cr query search '|New title' --filter 'app.main/comp-page' --exact
```

多匹配时 `search-replace` 会拒绝猜测；查看候选后用 `--pick <N>`，或改用 search → cursor → apply。

`--set-cursor` 会选中匹配 leaf。若要在它所在的表达式旁插入 sibling，先移动到 parent；插入后 cursor 仍跟随原表达式，再用 `next` 选中新节点：

```bash
cr query search '<leaf-in-expression>' --filter '<namespace/definition>' --exact --set-cursor 0
cr cursor parent
cr cursor apply insert-after --code 'quote $ <new-expression>'
cr --cursor-after focus cursor next
```

新增 definition 与 import 的最短路径：

```bash
cr edit add-ns app.util
cr edit def 'app.util/double' \
  --code 'quote $ defn double (x) (* x 2)'
cr edit add-import app.main \
  --code 'quote $ app.util :refer $ double'
cr query def 'app.util/double'
cr --check-only
```

`edit add-import` 接收一条 import rule body，不包含 `:require`；优先使用它。只有明确要整体替换全部 imports 时才使用 `edit imports`。已有 definition 或同来源 import 需要覆盖时必须显式加 `--overwrite`。优先局部 tree mutation，不要为了改几个节点整段覆盖。最后仍需运行项目规定的测试与 codegen。

## 4. Cursor 连续编辑

最常用的 cursor 循环只有四步：搜索选中、展示、修改、再展示。

```bash
cr query search render-item --filter 'app.main/render!' --exact
cr query search render-item --filter 'app.main/render!' --exact --set-cursor 0
cr cursor show
cr cursor apply wrap --code 'quote $ when visible? self'
cr cursor show
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

cursor 密集操作可把顶层选项写在命令前，如 `cr --cursor-after focus cursor forward --count 4`，让每次移动立即展示上下文。需要机器确认真实选中节点时，使用 `cr cursor show --format json --view node`。

`.calcit/` 是项目本地状态目录，应整体加入 `.gitignore`。其中 `cursor.cirru` 保存 active cursor、单一 anchor、最多 16 个 marks、last query、有限 history/stack 与 clipboard，硬上限 64 KiB；`error.cirru` 保存最近一次持久化 runtime/watcher stack，多行输入临时片段可放 `snippets/`。进入复制项目或已有 worktree 时先 `cursor show`；若选择与当前任务无关，执行 `cursor clear`。目前只有一个 active cursor，不负责多个进程的并发写入；并行 Agent 使用独立 worktree/Snapshot。

非法 cursor navigation 会保持 Snapshot 和 cursor 不变，失败后用 `cursor show` 确认。`unwrap` 会把所选 list 的所有 children splice 到 parent；对含额外语法的 wrapper，它不是 `wrap` 的撤销操作。

Paredit 的 slurp/barf、history/stack、结构化 clipboard、stale relocation 等细节按需查询：

```bash
cr docs read edit-tree.md 'Persistent Tree Cursor'
cr cursor --help
```

## 5. Cirru / Calcit 语法生存指南

先区分三个阶段：**Cirru 文本 → list/leaf AST → Calcit 求值**。`calcit.cirru` 再用 Cirru EDN 保存整个 Snapshot，其中 definition 的 `:code` 是 quoted AST。`cr cirru parse` 只证明文本能解析并展示树形；它不保证这棵树具有预期的 Calcit 语义。`cr cirru parse-edn` 查看的是 EDN 数据，不是同一个解析阶段。

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
- `[]`、`{}`、`#{}` 是 Calcit 的集合构造符号，不是 Clojure/JSON 的包围定界符。作为普通参数的裸 `[]` 是函数值，不是空 list；可靠写法是先 `init $ []` 再传 `init`，或显式使用 `([])`。
- `let` bindings 必须是 pair list：单行写 `let ((x 1)) ...`；多行时 bindings 比 body 多缩进一层：

  ```cirru.no-check
  let
      x 1
      y $ + x 1
    + x y
  ```

- `map`、`filter`、`foldl` 等 Calcit 集合函数把集合放在前面；不确定参数顺序时运行 `cr query examples calcit.core/map` 或查询对应定义，不要套用 Clojure 记忆。
- 改动缩进、`$`、`,` 或圆括号都会改变 AST 和 path；修改后重新 show/search，或继续使用 cursor。

### 5.3 CLI 的 `quote` 是代码/数据边界

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
| `--file <file>`       | 需要复用、审阅或 transaction 的输入 |
| 省略两者，从 stdin 读 | 一次性多行内容，避免 Shell 转义      |

修改命令没有 `--stdin` 参数。多行内容直接省略 `--file/--code`：

```bash
cr tree replace 'app.main/main!' --path '@3.1' <<'END'
quote $ if ready?
  render-ready
  render-loading
END
```

### 5.4 写入前先解析，写入后再做语义检查

```bash
cr cirru parse -e --validate 'a $ b c'
cr cirru parse --validate 'fn (x)
  , x'
cr cirru show-guide
```

`-e` 适合单行 expression 文本；多行 block 即使只表示一个 AST expression，也省略 `-e`。检查 JSON 形状是否符合预期后，再用 `cr eval`、`cr --check-only` 或项目测试验证 Calcit 语义。Shell 中 `$`、反引号、`|`、`>` 有特殊含义；短输入用单引号，多行 mutation 用 heredoc。

## 6. 验证与失败恢复

按从便宜到昂贵的顺序形成闭环：

1. 结构：`cursor show` 或 `tree show`，确认实际 subtree。
2. 语义：`query type-at ... --format json`，运行 `analyze check-types --summary-only`；看到 `W_TYPE_COVERAGE_GAPS` 后继续执行 `analyze weak-types --only schema-dynamic,code-dynamic --intent unresolved --summary-only`，再对命中范围去掉 `--summary-only` 查看 path、impact 与 suggestion。
3. definition：`analyze check-examples --ns <ns> --def <def>`。
4. 项目：运行仓库规定的 entry 和测试；只有项目目标是 JavaScript 时才运行对应的 `cr js` codegen。

`type-at` 的 unresolved/dynamic warning 只表示静态证据不足；`check-examples` 输出 `No functions with examples` 且退出 0 只表示没有 example 覆盖。二者都不是完成证明，仍要继续项目级 check、测试和目标 codegen。

不要用多个 `:dynamic` 假装多态：参数与返回共享类型时声明 `:generics`/TypeVar，只依赖能力时增加 trait `:where`，同质 collection/ref 保留 type arg，有限异构值使用 enum。`:any` 只是 `:dynamic` 的旧拼写，新 schema 和修复结果统一使用 `:dynamic`。只有明确的 FFI、global state 或 macro 边界保留 dynamic，并尽快在进入 typed code 时 validate/convert。

| 现象                   | 恢复动作                                                                 |
| ---------------------- | ------------------------------------------------------------------------ |
| `Invalid path`         | 重新 `query search`，或用 `--set-cursor`；不要继续复用旧坐标             |
| cursor stale/ambiguous | `cr cursor show` 检查；无法唯一恢复时重新 search + set                   |
| 输入解析失败           | `cr cirru parse -e` 预检；复杂输入改用 heredoc/文件                      |
| parse/preprocess/query/edit 失败 | 先以当前命令 stderr 为准；这些失败不保证刷新错误 sidecar                 |
| runtime/watcher stack  | `cr query error` 读取最近持久化栈；若提示 stale，不要追查其中的旧错误      |
| 多步修改范围不确定     | transaction `--dry-run --format json`，再带 `--expect-revision` 提交     |

优先用 `--set-cursor` 避免手工转换语义路径与 tree path。

## 7. 不知道时，通过 CLI 渐进查询

不要在本页复制完整能力地图，也不要凭记忆猜低频参数。先看 live help，再从 scope → 文件 → section 逐层读取：

```bash
cr --help
cr tree --help
cr tree replace --help

cr docs search 'cursor'
cr docs sections edit-tree.md
cr docs read edit-tree.md 'Persistent Tree Cursor'
cr docs read-lines agent-advanced.md --start 1 --lines 80
```

常见主题路由：

| 主题                         | 查询入口                                                        |
| ---------------------------- | --------------------------------------------------------------- |
| Cirru 语法、AST 与常见误写   | `cr cirru show-guide`；`cr docs read cirru-syntax.md 'Common Mistakes'` |
| Cirru EDN 数据层             | `cr cirru parse-edn --help`；`cr docs read edn.md --full`       |
| Calcit 与 Clojure 差异       | `cr docs read agent-advanced.md 'Calcit vs Clojure'`            |
| tree/cursor/transaction      | `cr docs read edit-tree.md --full`                              |
| query/context/type-at        | `cr docs read query.md --full`                                  |
| 复杂重构与历史陷阱           | `cr docs read agent-advanced.md --full`                         |
| Snapshot、deps 与项目结构    | `cr docs read project-structure.md --full`                      |
| 类型覆盖与 dynamic 审计      | `cr docs read static-analysis.md --full`；`cr analyze --help`   |
| 类库发布前验收与质量门禁     | `cr docs read library-quality.md --full`                       |
| run/watch/JS codegen         | `cr docs read cli-options.md 'Common Usage Patterns'`           |
| 错误排查                     | `cr docs read debugging.md --full`；`cr query error`            |
| 文档图与 frontmatter         | `cr docs read docs-indexing.md --full`；`cr docs graph --help`  |
| 安装模块的 API/示例          | `cr docs scopes` → `cr docs search <kw> --module <module>`      |

定义级资料优先从源码元数据查询：`cr query schema '<ns/def>'`、`cr query examples '<ns/def>'`、`cr query usages '<ns/def>'`。远程库发现、program diff、call graph、JS escape 等低频能力直接从对应 `--help` 开始。

## 8. 完成检查

- target 来自查询结果，不是猜测。
- 修改前看过目标 subtree，修改后重新 show/search。
- 没有把 `CURSOR`、`FOLDED:*`、chunk 或 path annotation 写回源码。
- 小改动没有整段 overwrite；多步原子修改使用 revision precondition。
- 运行了与改动范围和项目 target 匹配的验证。
- 没有修改依赖缓存、生成目录或无关文件。
