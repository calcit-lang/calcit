---
title: "Calcit Agent 快速实践（局部查看与编辑优先）"
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
---

# Calcit Agent 快速实践（局部查看与编辑优先）

本文档面向 Agent/LLM 的高频工作流，目标是**更快定位、最小改动、低噪音验证**。

本文定位为“查询与局部编辑速查表”：聚焦高频命令、路径定位和最小改动模板。执行前置约束与完整边界规则以 Agents 文档为准。

### 查询导航（先用这个）

- 看某个定义的大致结构：`cr query peek <ns/def>`
- 看某个定义的完整实现：`cr query def <ns/def>`
- 找关键词并拿可编辑路径：`cr query search <keyword> -f <ns/def>`
- 跨命名空间找符号：`cr query find <symbol>`（默认就是 fuzzy；需要精确匹配时加 `--exact`）
- 调试 JS 变量改名：`cr analyze js-escape '<symbol>'` / `cr analyze js-unescape '<escaped>'`（`js-unescape` 当前为 best-effort）
- 查进阶手册某个主题：`cr docs read agent-advanced.md <heading-keyword>`
- 看进阶手册全文：`cr docs read agent-advanced.md --full`

补充：仓库文件路径是 `docs/run/agent-advanced.md`，用 `cr docs read` 查询时文件名参数写 `agent-advanced.md`。

## Cirru 语法速览（先看这个）

结构化编辑依赖“树 + 路径”。先能读懂 Cirru，才能稳定算出路径坐标。

### Cirru 语法工具（`cr cirru`）

用于 Cirru 语法和 JSON 之间的转换：

- `cr cirru parse '<cirru_code>'` - 解析 Cirru 代码为 JSON
- `cr cirru format '<json>'` - 格式化 JSON 为 Cirru 代码
- `cr cirru parse-edn '<edn>'` - 解析 Cirru EDN 为 JSON
- `cr cirru show-guide` - 显示 Cirru 语法指南（帮助生成正确的 Cirru 代码）

**⚠️ 提示：如果你不确定某段缩进语法是否会被解析成预期结构，先运行一次 `cr cirru parse` 预检，再执行 `cr tree`/`cr edit` 修改。**

- Cirru 是缩进风格的 S-expression，缩进层级就是树层级。
- 行内空格分隔节点；嵌套表达式是子节点。
- 常见字面量：
  - `|text`：最常用的字符串写法。
  - 标准 one-liner 形式：`"|abc\nd"`（多行文本必须写成 `\n` 内嵌，不能直接跨行写字符串）。
  - `"|text with spaces"`：当字符串里有空格/特殊字符时，使用双引号前缀包裹整段 one-liner。
  - 双引号前缀不是通用替代：简单字符串优先 `|text`，只有在 `|...` 不够清晰时才用 `"|..."`。
  - `:tag`：tag
  - `[]` / `{}`：集合构造
- 你在 `cr query search` 里看到的 `[5.5.1.3]`，本质是“第 5 个子节点的第 5 个子节点的第 1 个子节点的第 3 个子节点”。

### 坐标如何从代码中读出来

示例表达式（简化）：

```cirru.no-check
defn demo (state)
  let
      result $ collect! state
    println result
```

- `query def` 先看全貌，不改。
- `query search collect! -f app.main/demo` 拿到路径（假设返回 `[3.1.2]`）。
- `tree show app.main/demo -p '3.1.2'` 验证该坐标确实是目标子树。
- 再做 replace/rewrite，避免“猜路径”。

### `$` 与 `,` 对坐标的影响（结合 Cirru 教程）

这两个符号都很常见，但它们对“树形坐标”的影响方式不同。

#### `$`：常常会改变树深度（更容易引起路径变化）

`$` 用于把右侧表达式折叠成一个子结构，通常会让目标节点进入更深一层。

```cirru.no-check
; "写法 A"
result $ collect! state

; "等价写法 B"
result (collect! state)
```

- 当你把一段调用改成/改掉 `$` 形式时，命中节点的路径经常会变深或变浅。
- 经验：改 `$` 之后，不复用旧路径，重新 `query search` 一次。

#### `,`：在“重起一行”场景里用于保持目标节点形态（有助于坐标稳定）

`,` 常用于告诉解析器“这里是值节点，不是再发起一次调用”。
在 Cirru 中，一行默认会被当作表达式；当你在表达式后另起一行并想表达“普通值”时，请写成 `, <value>`（逗号后有空格），避免被解析成新的调用。

```cirru.no-check
; "写法 A"
a (b c) d

; "等价写法 B"
a
  b c
  , d
```

- 在这组例子中，目标值 `d` 都是 `a` 的同级参数，通常可以视为同一坐标层级（只是写法不同）。
- 如果把 `, d` 误写成单独一行 `d`，它可能被解析成“调用形态”，节点类型会变化，后续路径与搜索命中也可能随之变化。
- 所以：`,` 本身通常不引入额外层级；它更多是在“换行写法”下保持你想要的 AST 形态。

#### Agent 生成前自检（20 秒）

- 字符串是否使用了 `|text` 或 `"|text with spaces"`，避免把字符串当符号。包含特殊字符需要双引号包裹.
- `let` 绑定是否是成对列表：`((name value))`，避免 `expects pairs in list for let`。
- 分支/函数最后一行若是“值”而非调用，是否使用了 `, value`。
- 只要对缩进有不确定，先用 `cr cirru parse '<code>'` 看 AST，再执行结构化编辑。

#### 先理解启动文件：`compact.cirru` 的 EDN 结构（Agent 快速模型）

Agent 切到新窗口时，优先把 `compact.cirru` 看成一个“可执行项目快照”，其顶层 EDN 结构通常是：

```cirru.no-check
{}
  :package |my-app
  :configs $ {}
    :init-fn |app.main/main!
    :reload-fn |app.main/reload!
    :modules $ [] |lilac/ |memof/
  :entries $ {}
    :test $ {}
      :init-fn |app.test/main!
      :reload-fn |app.test/reload!
      :modules $ [] |calcit-test/
  :files $ {}
    |app.main $ %{} :FileEntry
      :ns $ %{} :CodeEntry ...
      :defs $ {}
        |main! $ %{} :CodeEntry ...
```

字段职责可以快速记成：

- `:package`：包名边界（影响哪些 namespace 允许被 `cr edit` 修改）。
- `:configs`：默认运行入口（`cr` / `cr js` / `cr ir` 不指定 `--entry` 时使用）。
- `:entries`：命名入口集合（`cr --entry <name>` 走这里）。
- `:files`：源码数据库（namespace → `:ns` + `:defs`；每个定义是 `CodeEntry`，包含 code/doc/examples/schema）。
- `:modules`：加载的外部模块路径（通常来自 `~/.config/calcit/modules/`，目录结尾 `/` 默认补 `compact.cirru`）。

启动解析顺序（实操最常用）：

1. `cr`：使用 `:configs` 的 `:init-fn` / `:reload-fn` / `:modules`。
2. `cr --entry test`：切到 `:entries.test` 的配置运行。
3. `cr --init-fn xxx`：覆盖入口函数（常用于测试链路临时指定）。

建议每次开工先跑 3 条，建立项目运行心智：

```bash
cr query config
cr query ns <target-ns>
cr query defs <target-ns>
```

#### `deps.cirru` 与 `compact.cirru` 的关系（简版）

给 Agent 一个最小心智就够：

- `deps.cirru`：声明“要下载哪些外部模块 + 期望的 calcit 版本”。
- `compact.cirru`：声明“运行时要加载哪些模块（`:modules`）+ 项目代码快照（`:files`）”。

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

#### 实操规则（最稳）

凡是改到 `$` 或 `,`（尤其是从单行改成多行）时：

1. 先 `tree show` 看当前子树。
2. 修改后立刻 `query search <keyword> -f <ns/def>` 重拿路径。
3. 再继续下一步结构化编辑（`replace/wrap/rewrite`）。

## 0) 硬前置步骤

在任何 `cr edit` / `cr tree` 修改前，如果没有命令行相关的记忆, 执行命令获取关键文档的内容：

```bash
cr docs agents --full
```

这个文件默认存储在 `~/./config/calcit/Agents.md`, 后续步骤可以直接读取.

---

## 1) 默认约定（基于反馈）

- 默认优先 **Cirru 输出**，避免默认 JSON 带来的 token 膨胀。
- 大定义默认先 `query peek`，确认签名与规模后再 `query def`，避免首次信息过载。
- 路径统一使用点号：`'5.5.1.3'`。
- 大函数先“看结构再下刀”：先 `query def`，再 `query search` 拿路径，再 `tree show -p` 聚焦子树。
- 搜索命中很多时，修改遵循：
  - 从大索引往前改，或
  - 每次修改后重新 `query search` 避免路径漂移。
- Tips 需要但应可控：
  - 默认只在高优先级场景展示最多一条（快速扫读）
  - 需要全部提示时主动加 `--tips`
  - 需要精细控制时使用 `--tips-level`
- 涉及 `map/filter/reduce` 的改动，优先写成显式嵌套调用（`map xs f`、`filter xs pred`），再考虑 `->`，避免宏展开后参数位置误判。
- `query find` 不要再写 `-f/--fuzzy`（旧参数）；当前默认 fuzzy，需要精确匹配时使用 `--exact`。
- 在项目目录里用 `cr eval` 验证本项目定义时，默认不要加 `--dep ./`，避免重复加载本地模块导致 namespace 冲突。

> 说明：默认不加参数即 `minimal`（仅高优先级提示，最多 1 条）；`--tips` 等价于 `full`。也支持显式 `--tips-level minimal|full|none`。

---

## 2) 5 步最小模板（看大表达式并可编辑）

1. 定位目标定义：`cr query defs <ns>`
2. 先轻看再全看：`cr query peek <ns/def>`，必要时再 `cr query def <ns/def>`
3. 搜关键词拿路径：`cr query search <keyword> -f <ns/def>`
4. 聚焦子树确认上下文：`cr tree show <ns/def> -p '<path>'`（复杂时可加 `-j`；大表达式默认只展开 ROOT + 一层 chunks，需要更多时加 `--chunk-expand-depth 2`）
5. 修改并验证：`cr tree replace ...` 或 `cr edit inc --changed <ns/def>`，然后 `cr js`

> 修改时要求先考虑定位到坐标使用局部修改的方式, 或者结构化修改的方式, 若改动较大或不确定改动范围时再考虑整段覆盖式修改。

### 示例（大函数）

```bash
cr query peek respo.render.diff/find-element-diffs
cr query def respo.render.diff/find-element-diffs
cr query search collect! -f respo.render.diff/find-element-diffs
cr tree show respo.render.diff/find-element-diffs -p '5.5.1.3' -j
cr edit inc --changed respo.render.diff/find-element-diffs
cr js
```

---

## 3) 高频命令（只保留最常用）

### 查询

- `cr query defs <ns>`：列出命名空间定义。
- `cr query def <ns/def>`：查看定义（默认 Cirru）。
- `cr query search <pattern> -f <ns/def>`：按关键词拿路径。
- `cr tree show <ns/def> -p '<path>'`：查看局部子树；大表达式默认只显示 ROOT 与直接 chunk，继续展开时使用 `--chunk-expand-depth <n>`。

### 编辑

- `cr tree replace <ns/def> -p '<path>' -e '<code>'`：替换指定节点。
- `cr tree target-replace <ns/def> --pattern '<leaf>' -e '<code>' --leaf`：按内容唯一定位替换（优先）。
- `cr <snapshot-file> edit format`：按当前快照序列化逻辑重写 snapshot 文件，不改语义。
- `cr edit inc --changed <ns/def>`：增量编译当前修改定义。

`edit format` 用法例子：

```bash
cr src/cirru/calcit-core.cirru edit format
```

说明：`edit format` 作用于“当前输入 snapshot 文件”，在这个仓库里不要直接假设根目录有 `compact.cirru`。

### 小改动优先 `cr tree`（避免整段重置）

当需求只是“改少量内容或局部结构”时，**不要**先写完整文件再 `cr edit def --overwrite -f ...`。这会放大 token 消耗，也更容易引入无关漂移。

优先规则：

- 只改 1~10 个节点：优先 `cr tree` 系列。
- 仅改文本/叶子：优先 `target-replace` 或 `replace-leaf`。
- 只调单层结构：优先 `insert-*` / `delete` / `swap-*` / `wrap` / `raise`。
- 仅在“整段重写/新增定义/大范围重构”时，才用 `cr edit def --overwrite -f`。

典型场景模板：

1. 修改文本节点（leaf）

```bash
cr tree target-replace <ns/def> --pattern '|Old text' --leaf -e '|New text'
# 或
cr tree replace-leaf <ns/def> --from '|Old text' --to '|New text'
```

2. 删除节点

```bash
cr tree delete <ns/def> -p '<path>'
```

3. 一层表达式结构调整（同级顺序/包裹关系）

```bash
cr tree swap-next <ns/def> -p '<path>'
cr tree swap-prev <ns/def> -p '<path>'
cr tree wrap <ns/def> -p '<path>' -e 'when cond self'
cr tree raise <ns/def> -p '<child-path>'
```

4. 补充节点（插入 sibling/child）

```bash
cr tree insert-before <ns/def> -p '<path>' -e '<node>'
cr tree insert-after <ns/def> -p '<path>' -e '<node>'
cr tree insert-child <ns/def> -p '<path>' -e '<node>'
cr tree append-child <ns/def> -p '<path>' -e '<node>'
```

5. 每次小改后都做最小复核

```bash
cr tree show <ns/def> -p '<path>'
cr edit inc --changed <ns/def>
```

一句话：**小改动走 `cr tree`，大改动才整段覆盖。**

### 结构化策略（常用 5 招）

下面是“尽量不手写大段代码”的编辑策略，按风险从低到高使用。

#### 1) `cp`：复制现有子树，减少手输

```bash
cr tree cp app.main/demo --from '3.2' -p '4' --at after
```

- 含义：把路径 `3.2` 的子树复制到 `4` 后面。
- 适合：先复用旧逻辑，再做小改。

#### 2) `mv`：移动/重命名定义

```bash
cr edit mv app.main/old-name app.main/new-name
```

- 含义：定义级重命名或迁移。
- 适合：整理命名或模块边界。

#### 3) `wrap`：给目标套一层结构

```bash
cr tree wrap app.main/demo -p '5.2' -e 'when cond self'
```

- 含义：把原节点作为 `self` 嵌入新结构。
- 适合：快速加 guard、日志、转换壳。

#### 4) `raise`：提升子表达式，去掉中间壳

```bash
cr tree raise app.main/demo -p '5.2.1'
```

- 含义：用指定子节点替换其父节点。
- 适合：去掉多余 `let/when/pipe` 包裹层。

#### 5) `rewrite`：引用原节点做结构重排

```bash
cr tree rewrite app.main/demo -p '5.2' --with self=. -e '-> self normalize emit'
```

- 含义：在新模板中引用原节点（`.`）。
- 适合：复杂重构但希望保持局部语义。

> 实战建议：先 `target-replace/cp/wrap`，再用 `rewrite`；每步后 `tree show` 复核。

### 验证

- `cr edit format`: 重整快照文件，验证数据语法并格式化写法。
- `cr js`：快速验证当前改动可编译。
- 全量语义回归建议：`yarn check-all`。

---

## 4) 降噪与可读性建议

- 默认只看 Cirru，**必要时**才加 `-j`。
- 先 `query def` 看大轮廓，再 `search` + `tree show` 看局部。
- 搜索结果过多时，不要连续盲改路径；每次改后重搜一次更稳。
- 复杂多行表达式优先 `-f <file>`，减少 shell 转义错误。
- 默认模式通常不显示 tips；仅在高优先级场景显示 1 条。
- 若要看全部提示请加 `--tips`。
- 若要完全静默可用 `--tips-level none`。

### 本轮新增的稳定性约束（已验证）

- `cr query find` 当前默认 fuzzy，不再使用 `--fuzzy` / `-f` 这类旧参数；精确匹配用 `--exact`。
- Cirru 字符串统一按 one-liner 处理：多行文本用 `\n` 内嵌；含空格/特殊字符优先用 `"|text with spaces"`，简单字符串用 `|text`。
- 条件分支末尾若直接返回值（尤其 `nil`）出现调用歧义时，优先改成稳定值结构（例如 sentinel map）再做过滤。
- `cr query error` 提示旧错误堆栈时，先以本次 `cr js` / `cr --check-only` 结果为准，再决定是否继续追旧栈。

### 命令参数对照（易混）

- `cr query search <keyword> -f <ns/def>`：这里的 `-f` 是 `--filter`，用于限定搜索范围（有效）。
- `cr query find <symbol>`：默认就是 fuzzy，不再使用 `-f/--fuzzy`（无效）；精确匹配用 `--exact`。
- `cr edit def ... -f <file>`：这里的 `-f` 是“从文件读代码输入”（与 query 的 `-f` 含义不同）。

### `Invalid path` 快速恢复模板（固定 3 步）

当路径报错时，不要继续猜坐标，直接走下面流程：

1. `cr query search <keyword> -f <ns/def>` 重新拿最新路径。
2. `cr tree show <ns/def> -p '<new-path>'` 核对子树上下文。
3. 再执行 `tree replace/wrap/rewrite`。

常见触发原因：

- 前一步做了 `insert/delete/raise/unwrap`，兄弟索引已变化。
- 把单行改成多行（尤其涉及 `$`）后，子树深度发生变化。

### 低噪音工作模式（推荐给 Agent）

```bash
# 1) 先轻看，避免大段输出
cr query peek <ns/def>

# 2) 必要时才看完整定义（默认 Cirru）
cr query def <ns/def>

# 3) 用 search 定位后再 show 局部
cr query search '<keyword>' -f <ns/def>
cr tree show <ns/def> -p '<path>'

# 4) 需要完整提示再打开
cr --tips query def <ns/def>
```

仅在需要程序化处理时再加 `-j`，否则保持 Cirru 输出即可。

---

## 5) 路径规则（统一）

- 使用点号路径：`'5.5.1.3'`。
- `-p ''` 表示根节点，仅在明确需要根级操作时使用。
- 输入错误路径会触发 `Invalid path`，先 `tree show` 校验上下文再改。

---

## 6) 新手上手顺序（一次就够）

按顺序跑一遍即可建立手感：

```bash
cr query defs app.main
cr query def app.main/main!
cr query search state -f app.main/main!
cr tree show app.main/main! -p '3.2'
cr tree replace app.main/main! -p '3.2' -e 'new-expr'
cr edit inc --changed app.main/main!
cr js
```

---

## 7) 进阶入口（按需跳转）

本文件不重复收录低频内容，遇到下列场景再跳转：

- 复杂重构 / 大规模替换 / rewrite 组合：`cr docs read agent-advanced.md rewrite`
- 命名空间导入、输入格式与路径漂移陷阱：`cr docs read agent-advanced.md 命名空间`、`cr docs read agent-advanced.md 输入格式`
- 运行模式、eval 细节、CLI 约束：`Agents.md`
- 浏览所有可查文档：`cr docs list`
- 语言章节与 Cirru 语法细节：`cr docs read <file>` / `cr cirru show-guide`

---

## 8) 一句话原则

**先定位路径，再看子树，再最小替换；默认 Cirru，JSON 只在必要时启用。**

---

## 9) `cr` 能力地图（粗粒度）

当当前模板不够用时，按下面的“能力分层”自行扩展：

- 运行与编译：`cr`, `cr js`, `cr ir`, `cr wasm`（实验性）, `-w/--watch`
- 查询与定位：`cr query defs/def/search/search-expr/usages/schema/examples`
- 分析与影响评估：`cr analyze call-graph`, `cr analyze count-calls`
- 结构化编辑：`cr tree show/replace/target-replace/cp/wrap/unwrap/raise/rewrite`
- 定义级编辑：`cr edit mv/def/add-import/imports/...`
- 文档与指南：`cr docs list/read/search/agents`
- 语法学习：`cr cirru show-guide`

### Agent 自学习最短路径

```bash
cr docs list
cr analyze call-graph
cr analyze count-calls
cr docs search 'tree rewrite'
cr docs read run/edit-tree.md rewrite
cr docs search 'query search-expr'
```

原则：先在 docs 找“最小可行命令”，再回到当前定义做局部试改与验证。
