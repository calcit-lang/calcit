---
title: "Calcit 编程 Agent 指南"
scope: "core"
kind: "agent"
category: "run"
aliases:
  - "agent advanced"
  - "incremental edit"
  - "batch rename"
  - "agent playbook"
entry_for:
  - "cr query search"
  - "cr tree replace-leaf"
  - "cr edit inc"
---

# Calcit 编程 Agent 指南

本文档为 AI Agent 提供 Calcit 项目的操作指南。

本文定位为 Agents 约束与完整操作手册：覆盖硬前置步骤、命令边界、复杂重构与系统化排障。`docs/CalcitAgent.md` 用于查询与局部编辑速查，不替代本文中的约束规则。

## 🚀 快速开始（新 LLM 必读）

**硬前置步骤：在执行任何 `cr edit` / `cr tree` 修改前，必须先运行一次 `cr docs agents --full`。**

这不是建议项，而是进入实际修改前的检查项。跳过这一步，往往会直接沿用旧用法假设，尤其容易误判 `cr tree replace -p ''`、imports 输入格式和 watcher 验收边界。

**核心原则：用命令行工具（不要直接编辑文件），用 search 定位（比逐层导航快 10 倍）**

### 标准流程

```bash
# 搜索 → 修改 → 验证
cr query search 'symbol' -f 'ns/def'                      # 1. 定位（输出：[3.2.1] in ...）
cr tree replace 'ns/def' -p '3.2.1' --leaf -e 'new'     # 2. 修改
cr tree show 'ns/def' -p '3.2.1'                        # 3. 验证（可选）
```

### 三种搜索方式

```bash
cr query search 'target' -f 'ns/def'                      # 搜索符号/字符串
cr query search-expr 'fn (x)' -f 'ns/def'                 # 搜索代码结构
cr tree replace-leaf 'ns/def' --pattern 'old' -e 'new' --leaf  # 批量替换叶子节点
```

### 效率对比

| 操作       | 传统方法                | search 方法         | 效率     |
| ---------- | ----------------------- | ------------------- | -------- |
| 定位符号   | 逐层 `tree show` 10+ 步 | `query search` 1 步 | **10倍** |
| 查找表达式 | 手动遍历代码            | `search-expr` 1 步  | **10倍** |
| 批量重命名 | 手动找每处              | 自动列出所有位置    | **5倍**  |

---

## Tips 输出分级（已实现）

当前 CLI 已支持统一分级参数：`--tips-level`。

### 目标

- 默认保留必要引导，但降低噪音（首次扫读更快）。
- 在脚本/批处理与新手教学之间提供可切换策略。

### 建议枚举

- `--tips-level minimal`（默认）
  - 每次命令最多输出 1 条 tips（优先“下一步动作”）。
- `--tips-level full`
  - 输出全部 tips（教学/排障模式）。
  - 等价快捷参数：`--tips`
- `--tips-level none`
  - 关闭 tips（脚本/Agent 静默模式）。

### 使用建议

- 文档示例默认使用 `minimal` 心智模型，进阶示例再展示 `full/none`。

### 落地说明

1. 先在 query/tree 相关子命令接入统一解析。
2. 统一 Tips 渲染入口，避免各 handler 自行拼装。
3. 补充回归：默认输出条数、`full` 全量展示、`none` 静默行为。

---

## ⚠️ 重要警告：禁止直接修改的文件

以下文件**严格禁止使用文本替换或直接编辑**：

- **`calcit.cirru` / `compact.cirru`** - 这是 Calcit 程序的运行时快照格式；推荐使用 `calcit.cirru`，旧文件名 `compact.cirru` 仍兼容，必须使用 `cr edit` 相关命令进行修改

这两个文件的格式对空格和结构极其敏感，直接文本修改会破坏文件结构。请使用下面文档中的 CLI 命令进行代码查询和修改。

## Calcit 与 Cirru 的关系

- **Calcit** 是编程语言本身（一门类似 Clojure 的函数式编程语言）
- **Cirru** 是语法格式（缩进风格的 S-expression，类似去掉括号改用缩进的 Lisp）
- **关系**：Calcit 代码使用 Cirru 语法书写和存储

**具体体现：**

- `calcit.cirru`（兼容旧文件名 `compact.cirru`）使用 Cirru 语法存储, 尽量用 `cr edit` 和 `cr tree` 命令修改
- `cr cirru` 工具用于 Cirru 语法与 JSON 的转换（帮助理解和生成代码）
- Cirru 语法特点：
  - 用缩进代替括号（类似 Python/YAML）
  - 字符串用前缀 `|` 或 `"` 标记（如 `|hello` 表示字符串 "hello"）
  - 单行用空格分隔元素（如 `defn add (a b) (+ a b)`）

**类比理解：**

- Python 语言 ← 使用 → Python 语法
- Calcit 语言 ← 使用 → Cirru 语法

生成 Calcit 代码前，建议先运行 `cr cirru show-guide` 了解 Cirru 语法规则。

---

## Calcit CLI 命令

Calcit 程序使用 `cr` 命令：

### 主要运行命令

- `cr` 或 `cr calcit.cirru` - 代码解释执行，默认优先读取 `calcit.cirru`，找不到时回退到 `compact.cirru`（默认单次执行后退出）
- `cr -w` 或 `cr --watch` - 解释执行监听模式（显式启用监听）
- `cr calcit.cirru js` - 编译生成 JavaScript 代码（默认单次编译）
- `cr calcit.cirru js -w` / `cr calcit.cirru js --watch` - JS 监听编译模式
- `cr calcit.cirru ir` - 生成 program-ir.cirru（默认单次生成）
- `cr calcit.cirru ir -w` / `cr calcit.cirru ir --watch` - IR 监听生成模式
- `cr --check-only` - 仅检查代码正确性，不执行程序
  - 对 init_fn 和 reload_fn 进行预处理验证
  - 输出：预处理进度、warnings、检查耗时
  - 用于 CI/CD 或快速验证代码修改
- `cr js --check-only` - 检查代码正确性，不生成 JavaScript
- `cr --tips <subcommand> ...` - 主动显示完整 tips（教学/排障时）
  - 示例：`cr --tips calcit.cirru query def calcit.core/foldl`
- `cr eval '<code>' [--dep <module>...]` - 执行一段 Calcit 代码片段，用于快速验证写法
- - **不需要**项目运行时快照文件（`calcit.cirru` / `compact.cirru`）：core 内置函数（`range`、`+`、`map` 等）直接可用
- 项目自定义函数不可直接 eval（代码未加载），需用 `--dep` 加载外部模块
- `--dep` 参数可以加载 `~/.config/calcit/modules/` 中的模块（直接使用模块名），可多次使用
- 示例：`cr eval 'range 5'`、`cr eval 'echo 1' --dep calcit.std`

### 查询子命令 (`cr query`)

这些命令用于查询项目信息：

**项目全局分析：**

- `cr analyze call-graph` - 分析从入口点开始的调用图结构
- `cr analyze count-calls` - 统计每个定义的调用次数

  _使用示例：_

  ```bash
  # 分析整个项目的调用图
  cr analyze call-graph
  # 统计调用次数
  cr analyze count-calls
  ```

**基础查询：**

- `cr query ns [--deps]` - 列出项目中所有命名空间（--deps 包含依赖）
- `cr query ns <namespace>` - 读取命名空间详情（imports, 定义预览）
- `cr query defs <namespace>` - 列出命名空间中的定义
- `cr query pkg` - 获取项目包名
- `cr query config` - 读取项目配置（init_fn, reload_fn, version）
- `cr query error` - 读取 .calcit-error.cirru 错误堆栈文件
- `cr query modules` - 列出项目依赖的模块（来自 calcit.cirru 配置，兼容旧文件名 `compact.cirru`）

**渐进式代码探索（Progressive Disclosure）：**

- `cr query peek <namespace/definition>` - 查看定义签名（参数、文档、表达式数量），不返回完整实现体
  - 输出：Doc、Form 类型、参数列表、Body 表达式数量、首个表达式预览、Examples 数量
  - 用于快速了解函数接口，减少 token 消耗
- `cr query def <namespace/definition> [-j]` - 读取定义的完整 Cirru 代码
  - 默认输出：Doc、Examples 数量、Cirru 格式代码
  - `-j` / `--json`：同时输出 JSON 格式（用于程序化处理）
  - 推荐：LLM 直接读取 Cirru 格式即可，通常不需要 JSON
- `cr query schema <namespace/definition> [-j]` - 读取定义当前的 schema
  - 默认输出：Definition 标识 + schema 的 Cirru one-liner 预览
  - `-j` / `--json`：输出 schema 对应的 Cirru EDN 结构；无 schema 时输出 `nil`
  - 适合在修改前确认 `:args` / `:return` / `:rest` 当前值
- `cr query examples <namespace/definition>` - 读取定义的示例代码
  - 输出：每个 example 的 Cirru 格式和 JSON 格式

**符号搜索与引用分析：**

- `cr query find <symbol> [--deps] [-f] [-n <limit>]` - 跨命名空间搜索符号
  - 默认精确匹配：返回定义位置 + 所有引用位置（带上下文预览）
  - `-f` / `--fuzzy`：模糊搜索，匹配 "namespace/definition" 格式的路径
  - `-n <limit>`：限制模糊搜索结果数量（默认 20）
  - `--deps`：扩展到 `calcit.*` 内置核心命名空间（默认已包含项目 modules 依赖）
- `cr query usages <namespace/definition> [--deps]` - 查找定义的所有使用位置
  - 返回：引用该定义的所有位置（带上下文预览）
  - 用于理解代码影响范围，重构前的影响分析
  - `--deps`：同上，扩展到 `calcit.*` 核心命名空间

**代码模式搜索（快速定位 ⭐⭐⭐）：**

- `cr query search <pattern> [-f <filter>] [--exact]` - 搜索叶子节点（符号/字符串），比逐层导航快 10 倍
  - **搜索范围**：默认包含项目代码、全部 modules 依赖和 calcit.core 内置函数（无需 `--deps` 标志）
  - `--entry <name>`：额外加载 `entries.<name>.modules` 里的依赖（用于 entry 级依赖场景）
  - `-f <filter>` - 过滤到特定命名空间或定义（可缩小范围提升速度）
  - 默认即为 contains / fuzzy 匹配
  - `--exact`：仅匹配完全相等的叶子节点
  - `-d <max-depth>`：限制搜索深度
  - `-p <start-path>`：从指定路径开始搜索（如 `"3.2.1"`，也兼容 `"3,2,1"`）
  - 返回：完整路径 + 父级上下文，多个匹配时自动显示批量替换命令
  - 示例：
    - `cr query search 'println' -f app.main/main!` - 默认 contains 匹配（过滤到某定义）
    - `cr query search --exact 'println' -f app.main/main!` - 精确搜索
    - `cr query search 'comp-' -f app.ui/layout` - 模糊搜索（所有 comp- 开头）
    - `cr query search 'task-id'` - 全项目搜索（含 modules）

**高级结构搜索（搜索代码结构 ⭐⭐⭐）：**

- `cr query search-expr <pattern> [-f <filter>] [--exact] [-j]` - 搜索结构表达式（List）
  - **搜索范围**：同 `search`，默认包含全部依赖和 calcit.core
  - `--entry <name>`：同上，额外加载指定 entry 的 modules
  - 默认即为 prefix / contains 匹配（嵌套表达式也支持前缀）
  - `--exact`：仅匹配结构完全一致的表达式
  - `-j / --json`：将模式解析为 JSON 数组
  - 示例：
    - `cr query search-expr 'fn (x)' -f app.main/process` - 查找函数定义
    - `cr query search-expr --exact 'fn (x)' -f app.main/process` - 仅匹配结构完全一致的表达式
    - `cr query search-expr '>> state task-id'` - 查找状态访问（匹配 `>> state task-id ...` 或 `>> state`）
    - `cr query search-expr 'dispatch! (:: :states)'` - 匹配 `dispatch! (:: :states data)` 类型的表达式
    - `cr query search-expr 'memof1-call-by'` - 查找记忆化调用

**搜索结果格式：** `[索引1.索引2...] in 父级上下文`，可配合 `cr tree show <ns/def> -p '<path>'` 查看节点。逗号路径仍兼容，但文档与输出优先使用点号。**修改代码时优先用 search 命令，比逐层导航快 10 倍。**

### LLM 辅助：动态方法提示

在运行时调试 trait 分派时，可使用以下内置函数（低频场景，需运行期有值后调用）：

- `&methods-of value` — 列出某值的可用方法名（返回字符串列表 `[] |.foo |.bar ...`）
- `&inspect-methods value` — 打印方法与 impl 来源（调试 trait override 链，可临时插入 pipeline）
- `&impl:origin impl` — 读取 impl record 的 trait 来源
- `&trait-call Trait :method receiver & args` — 显式消歧：只调用属于指定 trait 的方法实现

> 📖 深入了解 trait 实现机制：`cr docs read traits.md` 或 `cr docs search 'trait-call'`

### 文档子命令 (`cr docs`)

文档查询采用渐进披露的四层路径：scope -> file -> sections -> content。

- `cr docs scopes` - 列出可查 scope（`calcit` 与已安装模块目录名）
- `cr docs list [--module <name>]` - 列出当前 scope 下的文档文件
- `cr docs sections <filename> [--module <name>]` - 列出某个文件中的章节标题
- `cr docs read <filename> [<heading> ...] [--module <name>]` - 读取整份文档，或按标题关键词跳到章节

- `cr docs search <keyword> [-c <num>] [-f <filename>] [--module <name>]` - 按关键词搜索文档内容
  - `-c <num>` - 显示匹配行的上下文行数（默认 5）
  - `-f <filename>` - 按文件名过滤搜索结果
  - `--module <name>` - 搜索某个已安装模块文档
  - 输出：匹配行及其上下文，带行号和高亮
  - 示例：`cr docs search 'macro' -c 10` 或 `cr docs search 'defn' -f macros.md`

- `cr docs read <filename> [<heading> ...] [--module <name>]` - 按 Markdown 标题阅读文档
  - 不传标题时：读取整份文档内容
  - 传入一个或多个标题关键词时：按标题做模糊匹配并输出对应章节内容
  - 示例：`cr docs read macros.md`、`cr docs read Respo-Agent --module respo.calcit`

- `cr docs read-lines <filename> [-s <start>] [-n <lines>] [--module <name>]` - 按行读取文档（兼容旧行为）
  - `-s <start>` - 起始行号（默认 0）
  - `-n <lines>` - 读取行数（默认 80）
  - 输出：文档内容、当前范围、是否有更多内容
  - 示例：`cr docs read-lines intro.md -s 20 -n 30`

- `cr docs agents [<heading> ...] [--full]` - 读取 Agent 指南（即本文档，优先本地缓存，按天自动刷新）
  - 不传标题时列出所有标题；传关键词时按标题模糊匹配输出对应章节

### Cirru 语法工具

`cr cirru parse/format/parse-edn/show-guide` 的高频命令已收敛到 `docs/CalcitAgent.md` 的「Cirru 语法速览」章节，便于在局部编辑工作流中直接查用。

若缩进结构不确定，先执行 `cr cirru parse '<cirru_code>'` 预检 AST/JSON，再继续 `cr tree` 或 `cr edit` 修改。

### 远程库文档 (`cr docs remote-libs`)

查询和了解 Calcit 官方库：

- `cr docs remote-libs` - 列出所有官方库
- `cr docs remote-libs search <keyword>` - 按关键词搜索库（搜索名称、描述、分类）
- `cr docs remote-libs readme <package> [-f <file>]` - 查看库的文档
  - 优先从本地 `~/.config/calcit/modules/<package>` 读取
  - 本地不存在时从 GitHub 仓库获取
  - `-f` 参数可指定其他文档文件（如 `-f Skills.md`）
  - 默认读取 `README.md`
- 模块文档查询统一优先使用 `cr docs scopes/list/sections/read/search ...`
- `cr docs remote-libs scan-md <module>` - 兼容性低层入口，按目录扫描本地模块下的 `.md` 文件
- `caps` - 安装/更新依赖（默认读取 `deps.cirru`，也可传自定义文件路径）
  - 独立工具（非 `cr` 子命令）
  - `caps`：按 `deps.cirru` 当前依赖执行更新
  - `caps add <group>/<repo>`：添加依赖并执行默认更新流程
  - `caps remove <group>/<repo>`：移除依赖并执行默认更新流程
  - `caps add/remove` 同时支持完整 GitHub 地址（如 `https://github.com/calcit-lang/memof`）
  - `caps add -r <version>`：写入指定分支/版本（默认 `main`）

**查看已安装模块：**

```bash
# 列出 ~/.config/calcit/modules/ 下所有已安装的模块
ls ~/.config/calcit/modules/

# 查看当前项目配置的模块依赖
cr query modules
```

### 精细代码树操作 (`cr tree`)

⚠️ **关键警告：路径索引动态变化**

删除或插入节点后，同级后续节点的索引会自动改变。**必须从后往前操作**或**每次修改后重新搜索路径**。

**核心概念：**

- 路径格式：优先使用点号分隔的索引（如 `"3.2.1"`），逗号写法 `"3,2,1"` 仍兼容；空字符串 `""` 表示根节点
- `-p ''` 仅表示“根节点”，**不等于推荐的整定义重写方案**；要整体替换定义时，优先使用 `cr edit def --overwrite -f <file>`
- 每个命令都有 `--help` 查看详细参数
- 命令执行后会显示 "Next steps" 提示下一步操作

**主要操作：**

- `cr tree show <ns/def> -p '<path>' [-j]` - 查看节点
  - 默认输出：节点类型、Cirru 预览、操作提示
  - 大表达式分片时默认只展开 ROOT 与直接 chunk；需要继续展开嵌套 chunk 时加 `--chunk-expand-depth <n>`
  - `-j` / `--json`：同时输出 JSON 格式（用于程序化处理）
  - 推荐：直接查看 Cirru 格式即可，通常不需要 JSON
- `cr tree replace` - 替换节点
  - 适合局部节点替换；若目标是**整条定义**，优先改用 `cr edit def --overwrite -f <file>`，比 `cr tree replace -p ''` 更可预期
- `cr tree replace-leaf` - 查找并替换所有匹配的 leaf 节点（无需指定路径）
  - `--pattern <pattern>` - 要搜索的模式（精确匹配 leaf 节点）
  - 使用 `-e, -f, -j` 等通用参数提供替换内容
  - 自动遍历整个定义，一次性替换所有匹配项
  - 示例：`cr tree replace-leaf 'ns/def' --pattern 'old-name' -e 'new-name' --leaf`
- `cr tree search-replace` - 基于内容的唯一替换（无需指定路径，更安全 ⭐⭐⭐）
  - `--pattern <pattern>` - 要搜索的模式（精确匹配 leaf 节点）
  - 使用 `-e, -f, -j` 等通用参数提供替换内容
  - 逻辑：自动查找叶子节点，若唯一则替换；若不唯一则报错并列出所有位置及修改命令建议。
- `cr tree delete <ns/def> -p '<path>'` - 删除指定路径节点（⚠️ 后续同级索引自动减小）
  - 示例：`cr tree delete app.core/fn -p '3.2'`
- `cr tree insert-before <ns/def> -p '<path>'` / `cr tree insert-after` - 在路径节点的前/后插入兄弟节点
  - 示例：`cr tree insert-before app.core/fn -p '3.2' -e 'new-expr'`
- `cr tree insert-child <ns/def> -p '<path>'` / `cr tree append-child` - 在某节点内部最前/最后插入子节点
  - 示例：`cr tree append-child app.core/fn -p '3' --leaf -e 'new-arg'`
- `cr tree swap-next <ns/def> -p '<path>'` / `cr tree swap-prev` - 将节点与其下一个/上一个兄弟节点交换位置
- `cr tree rewrite` - 用引用原节点的新结构替换节点（`--with` 必须；需引用子节点时使用）
- `cr tree wrap` - 快捷包裹节点：将 `self` 替换为原节点（`rewrite --with self=.` 的语法糖）
- `cr tree unwrap` - 将节点的所有子节点展开拼接到父节点中（拆包），原节点消失
- `cr tree raise` - 用指定子节点替换其父节点（Paredit raise-sexp）

**输入方式（通用）：**

- `-e '<code>'` - 内联代码（自动识别 Cirru/JSON）
- `--leaf` - 强制作为 leaf 节点（符号或字符串）
- `-j '<json>'` / `-f <file>`

简单更新尽量用结构化的 API 操作. 多行或者带特殊符号的表达式, 可以在 `.calcit-snippets/` 创建临时文件, 然后用 `cr cirru parse` 验证语法, 最后用 `-f <file>` 提交, 从而减少错误率. 复杂表达式建议分段, 然后搭配 `cr tree search-replace` 命令来完成多阶段提交.

**整体替换定义的经验规则：**

- 局部节点修改：继续使用 `cr tree replace -p '<path>'`
- 整条定义重写：优先使用 `cr edit def <ns/def> --overwrite -f <file>`
- 只有在你明确知道根节点替换后的结构，并且能立刻验证完整定义时，才考虑 `cr tree replace -p ''`

**推荐工作流（高效定位 ⭐⭐⭐）：**

```bash
# ===== 方案 A：单点修改（精确定位） =====

# 1. 快速定位目标节点（一步到位）
cr query search 'target-symbol' -f namespace/def
# 输出：[3.2.5.1] in (fn (x) target-symbol ...)

# 2. 直接修改（路径已知）
cr tree replace namespace/def -p '3.2.5.1' --leaf -e 'new-symbol'

# 3. 验证结果（可选）
cr tree show namespace/def -p '3.2.5.1'


# ===== 方案 B：批量重命名（多处修改） =====

# 1. 搜索所有匹配位置
cr query search 'old-name' -f namespace/def
# 自动显示：4 处匹配，已按路径从大到小排序
# [3.2.5.8] [3.2.5.2] [3.1.0] [2.1]

# 2. 按提示从后往前修改（避免路径变化）
cr tree replace namespace/def -p '3.2.5.8' --leaf -e 'new-name'
cr tree replace namespace/def -p '3.2.5.2' --leaf -e 'new-name'
# ... 继续按序修改

# 或：一次性替换所有匹配项
cr tree replace-leaf namespace/def --pattern 'old-name' -e 'new-name' --leaf


# ===== 方案 C：基于内容的半自动替换（最推荐 ⭐⭐⭐） =====

# 1. 尝试基于叶子节点内容直接替换
cr tree search-replace namespace/def --pattern 'old-symbol' -e 'new-symbol' --leaf

# 2. 如果存在多个匹配，命令会报错并给出详细指引（包含具体路径的 replace 命令建议）
# 如果确定要全部替换，可改用 tree replace-leaf


# ===== 方案 D：结构搜索（查找表达式） =====

# 1. 搜索包含特定模式的表达式
cr query search-expr "fn (task)" -f namespace/def
# 输出：[3.2.2.5.2.4.1] in (map $ fn (task) ...)

# 2. 查看完整结构（可选）
cr tree show namespace/def -p '3.2.2.5.2.4.1'

# 3. 修改整个表达式或子节点
cr tree replace namespace/def -p '3.2.2.5.2.4.1.2' -e 'let ((x 1)) (+ x task)'
```

**关键技巧：**

- **优先使用 `search` 系列命令**：比逐层导航快 10+ 倍，一步直达目标
- **路径格式**：`"3.2.1"` 表示第3个子节点 → 第2个子节点 → 第1个子节点；`"3,2,1"` 仍兼容
- **批量修改自动提示**：搜索找到多处时，自动显示路径排序和批量替换命令
- **路径动态变化**：删除/插入后，同级后续索引会变化，按提示从后往前操作
- **批量执行不要用 `&&` 粘成一行**：尤其当 `-e` 内容里有引号、`|` 字符串或复杂表达式时，优先逐条执行，或写入 `-f <file>` 避免 shell 进入未闭合引号状态
- 所有命令都会显示 Next steps 和操作提示

**结构化变更示例：**

`cr tree rewrite` 用于在替换时引用原节点及其子节点，必须传至少一个 `--with name=path`（不需要引用时直接用 `replace`）。`--with` 格式：`name=path`，`.` 表示原节点本身，数字表示子节点索引。

- **包裹节点**（推荐用 `wrap`，`self` 作为占位符）：

  ```bash
  # 将路径 "3.2" 的节点包裹在 println 中（self = 原节点）
  cr tree wrap ns/def -p '3.2' -e 'println self'

  # 等价的完整写法（需要引用子节点时才用 rewrite）
  cr tree rewrite ns/def -p '3.2' -e 'println self' -w 'self=.'
  ```

- **引用原节点局部**（`rhs=2` 引用子节点索引 2）：
  - 假设原节点是 `+ 1 2`（路径 `3.1`），子节点索引 2 是 `2`
  - 将其重构为 `* rhs 10`：

  ```bash
  cr tree rewrite ns/def -p '3.1' -e '* rhs 10' -w 'rhs=2'
  ```

- **多处重用原节点**：

  ```bash
  # 将节点变为 `+ x x`（self 引用原节点本身）
  cr tree rewrite ns/def -p '2' -e '+ self self' -w 'self=.'
  ```

- **拆包节点**（`unwrap`）——将节点的所有子节点展开拼接到父节点中，原节点消失：

  ```bash
  # 将路径 "3.2" 的节点拆包，所有子节点直接插入到原位置
  cr tree unwrap ns/def -p '3.2'
  ```

  详细参数和示例使用 `cr tree <command> --help` 查看。

- **提升子节点替换父节点**（`raise`）——用某子节点整体替换掉其父节点（Paredit `raise-sexp`）：

  ```bash
  # 路径 "3.2" 的节点整体替换掉其父节点 "3"
  # 使用场景：去掉 let 外层只保留返回值，或去掉 if 只保留 then/else 分支
  cr tree raise ns/def -p '3.2'
  ```

- **提取子表达式为新定义**（`split-def`）——将某路径的子表达式提取为同 ns 的新定义，原位替换为新名字：

  ```bash
  # 将路径 "3.2" 的子表达式提取为新定义 compute-helper（同 namespace）
  cr edit split-def app.util/process -p '3.2' -n compute-helper
  ```

  详细参数使用 `cr edit split-def --help` 查看。

### 复杂表达式分段组装策略 (Incremental Assembly) ⭐⭐⭐

当需要构造非常复杂的嵌套结构（例如递归循环、多级 `let` 或 `if`）时，直接通过 `-e` 传入单行 Cirru 代码容易遇到 shell 转义、括号对齐或长度限制等问题。推荐使用**分段占位组装**策略：

简单提示：

- 占位符统一使用 `{{NAME}}` 风格，例如 `{{BODY}}`、`{{TRUE_BRANCH}}`；
- 大表达式可以先用 `cr query def <ns/def>` 看整体分片，再用 `cr tree show <ns/def> -p '<path>'` 深入某个片段；
- 真正填充时，优先用 `cr tree search-replace` 找占位符，不唯一时再退回路径替换。

1. **确立骨架**：先替换目标节点为一个带有占位符的简单 JSON 结构。

   ```bash
   cr tree replace ns/def -p '4.0' -j '["let", [["x", "1"]], "{{BODY}}"]'
   ```

2. **定位占位符**：使用 `tree show` 确认占位符的具体路径。

   ```bash
   cr tree show ns/def -p '4.0'
   ```

# 输出显示 "{{BODY}}" 在索引 2，即路径 [4.0.2]

````

3. **填充内容**：针对占位符路径进行下一层的精细替换。

```bash
cr tree replace ns/def -p '4.0.2' -j '["if", ["=", "x", "1"], "{{TRUE_BRANCH}}", "{{FALSE_BRANCH}}"]'
````

4. **递归迭代**：重复上述步骤直到所有占位符（如 `{{TRUE_BRANCH}}`、`{{FALSE_BRANCH}}`）都被替换为最终逻辑。

**优势：**

- **精确性**：使用 JSON 格式 (`-j`) 可以完全避免 Cirru 缩进或括号解析的歧义。
- **低风险**：每次只修改一小部分，出错时容易通过 `tree show` 快速定位。
- **绕过限制**：解决某些终端对超长命令行参数的限制。

### 代码编辑 (`cr edit`)

直接编辑 calcit.cirru 项目代码（兼容旧文件名 `compact.cirru`），支持两种输入方式：

- `--file <path>` 或 `-f <path>` - 从文件读取（默认 Cirru 格式，使用 `-J` 指定 JSON）
- `--json <string>` 或 `-j <string>` - 内联 JSON 字符串

额外支持“内联代码”参数：

- `--code <text>` 或 `-e <text>`：直接在命令行里传入一段代码。
  - 默认按 **Cirru 单行表达式（one-liner）** 解析。
  - 如果输入“看起来像 JSON”（例如 `-e '"abc"'`，或 `-e '["a"]'` 这类 `[...]` 且包含 `"`），则会按 JSON 解析。
  - ⚠️ 当输入看起来像 JSON 但 JSON 不合法时，会直接报错（不会回退当成 Cirru one-liner）。

对 `--file` 输入，还支持以下“格式开关”（与 `-J/--json-input` 类似）：

- `--leaf`：把输入当成 **leaf 节点**，直接使用 Cirru 符号或 `|text` 字符串，无需 JSON 引号。
  - 传入符号：`-e 'my-symbol'`
  - 传入字符串：加 Cirru 字符串前缀 `|` 或 `"`，例如 `-e '|my string'` 或 `-e '"my string'`

⚠️ 注意：这些开关彼此互斥（一次只用一个）。

**推荐简化规则（命令行更好写）：**

- **JSON（单行）**：优先用 `-j '<json>'` 或 `-e '<json>'`（不需要 `-J`）。
- **Cirru 单行表达式**：用 `-e '<expr>'`（`-e` 默认按 one-liner 解析）。
- **Cirru 多行缩进**：用 `-f file.cirru`。
- `-J/--json-input` 主要用于 **file** 读入 JSON（如 `-f code.json -J`）。

补充：`-e/--code` 只有在 `[...]` 内部包含 `"` 时才会自动按 JSON 解析（例如 `-e '["a"]'`）。
像 `-e '[]'` / `-e '[ ]'` 会默认按 Cirru one-liner 处理；如果你需要“空 JSON 数组”，用显式 JSON：`-j '[]'`。

如果你想在命令行里明确“这段就是 JSON”，请用 `-j '<json>'`（`-J` 是给 file 用的）。

**定义操作：**

- `cr edit format` - 不修改语义，按当前快照序列化逻辑重写 **snapshot 文件**（用于刷新格式）
  - 也会把旧的 namespace `CodeEntry` 写法收敛成当前的 `NsEntry` 结构
  - 适用：普通 `calcit.cirru` / 项目 snapshot 文件（兼容旧文件名 `compact.cirru`）
  - 不适用：calcit-editor 专用的 `calcit.cirru` 结构文件
- `cr edit def <namespace/definition>` - 添加新定义（默认若已存在会报错；加 `--overwrite` 可强制覆盖）
  - 经验语义：**不带 `--overwrite` = create-only；带 `--overwrite` = replace existing definition**
  - 若当前输出文案仍显示 `Created definition`，以你的调用方式和目标是否已存在为准理解，不要把该提示字面理解为“必然新增成功”
- `cr edit rename <namespace/definition> <new-name>` - 在当前命名空间内重命名定义（不可覆盖）
- `cr edit mv-def <source> <target>` - 将定义移动到另一个命名空间（跨命名空间移动）
- `cr edit cp <ns/def> --from <path> -p <path> [--at <pos>]` - 在定义内复制 AST 节点到另一位置
- `cr edit mv <ns/def> --from <path> -p <path> [--at <pos>]` - 在定义内移动 AST 节点（复制后删除原位置；自动防止移入自身子树）
- `cr edit split-def <ns/def> -p <path> -n <new-name>` - 将定义内某路径的子表达式提取为同命名空间内的新定义，原位置替换为新定义名称（新名称不可与已有定义重名）
- `cr edit rm-def <namespace/definition>` - 删除定义
- `cr edit doc <namespace/definition> '<doc>'` - 更新定义的文档
- `cr edit schema <namespace/definition>` - 更新定义 schema（写入前会校验 schema 结构）
  - 常用输入：`-e ':: :fn $ {} (:args $ [] :number :number) (:return :number)'`
  - 也支持 `-f <file>` / `-j '<json>'` / `-J` / `--leaf`
  - `--clear`：清空 schema，恢复为 `nil`
  - 写入后会保存为直接 map 形式；后续运行与 preprocess 会用它做 `defn` / `defmacro` 一致性校验
- `cr edit examples <namespace/definition>` - 设置定义的示例代码（批量替换）
- `cr edit add-example <namespace/definition>` - 添加单个示例
- `cr edit rm-example <namespace/definition> <index>` - 删除指定索引的示例（0-based）

**命名空间操作：**

> ⚠️ **关键：各命令的 `-e` 期望格式不同，不可混用，详见下方「命名空间操作陷阱」**

- `cr edit add-ns <namespace>` - 添加命名空间
  - 无 `-e`：创建空 ns（推荐；再用 `add-import` 逐条添加）
  - `-e 'ns my.ns $ :require ...'`：需传完整 `ns` 表达式，名称必须与位置参数一致
- `cr edit rm-ns <namespace>` - 删除命名空间
- `cr edit imports <namespace>` - 更新导入规则（**全量替换**所有 import）
  - `-e 'source-ns :refer $ sym1 sym2'`：单条规则（**不含** `:require` 前缀）
  - `-f rules.cirru`：多条规则文件，每行一条（推荐多条场景）
  - `-j '[["src-ns",":refer",["sym"]],...]'`：JSON 数组格式，每元素为一条规则
- `cr edit add-import <namespace>` - 添加单个 import 规则（**不替换**已有规则）
  - `-e 'source-ns :refer $ sym1 sym2'`：单条规则
  - `-o` / `--overwrite`：覆盖已存在的同名源 ns 规则
- `cr edit rm-import <namespace> <source_ns>` - 移除指定来源的 import 规则
- `cr edit ns-doc <namespace> '<doc>'` - 更新命名空间文档

**模块和配置（统一使用 `cr config`）：**

- `cr config add-module <module-path>` - 添加模块依赖
- `cr config rm-module <module-path>` - 删除模块依赖
- `cr config set <key> <value>` - 设置配置（key: init-fn, reload-fn, version）
- `cr config version patch|minor|major` - 递增项目语义化版本号

**增量变更导出：**

- `cr edit inc` - 记录增量代码变更并导出到 `.compact-inc.cirru`，触发 watcher 热更新
  - `--added <namespace/definition>` - 标记新增的定义
  - `--changed <namespace/definition>` - 标记修改的定义
  - `--removed <namespace/definition>` - 标记删除的定义
  - TIP: 使用 `cr edit mv` 移动定义后，需手动执行 `cr edit inc --removed <source> --added <target>` 以更新 watcher。
  - `--added-ns <namespace>` - 标记新增的命名空间
  - `--removed-ns <namespace>` - 标记删除的命名空间
  - `--ns-updated <namespace>` - 标记命名空间导入变更
  - 配合 watcher 使用实现热更新（详见"开发调试"章节）

使用 `--help` 参数了解详细的输入方式和参数选项。

---

## Calcit 语言基础

### Cirru 语法核心概念

**与其他 Lisp 的区别：**

- **缩进语法**：用缩进代替括号（类似 Python/YAML），单行用空格分隔
- **字符串前缀**：`|hello` 或 `"hello"` 表示字符串，`|` 前缀更简洁
- **无方括号花括号**：只用圆括号概念（体现在 JSON 转换中），Cirru 文本层面无括号

**常见混淆点：**

❌ **错误理解：** Calcit 字符串是 `"x"` → JSON 是 `"\"x\""`  
✅ **正确理解：** Cirru `|x` → JSON `"x"`，Cirru `"x"` → JSON `"x"`

**字符串 vs 符号的关键区分：**

- `|Add` 或 `"Add` → **字符串**（用于显示文本、属性值等, 前缀形式区分字面量类型）
- `Add` → **符号/变量名**（Calcit 会在作用域中查找）
- 常见错误：受其他语言习惯影响，忘记加 `|` 前缀导致 `unknown symbol` 错误

**CLI 使用提示：**

- 替换包含空格的字符串：`--leaf -e '|text with spaces'` 或 `-j '"text"'`
- 避免解析为列表：字符串字面量必须用 `--leaf` 或 `-j` 明确标记

**示例对照：**

| Cirru 代码       | JSON 等价                        | JavaScript 等价          |
| ---------------- | -------------------------------- | ------------------------ |
| `\|hello`        | `"hello"`                        | `"hello"`                |
| `"world"`        | `"world"`                        | `"world"`                |
| `\|a b c`        | `"a b c"`                        | `"a b c"`                |
| `fn (x) (+ x 1)` | `["fn", ["x"], ["+", "x", "1"]]` | `fn(x) { return x + 1 }` |

### 数据结构：Tuple vs Vector

Calcit 特有的两种序列类型：

**Tuple (`::`)** - 不可变、用于模式匹配

```cirru.no-check
; 创建 tuple
:: :event/type data

; plain tuple 也可以直接用原生 match
match event
  (:event/click data) (handle-click data)
  (:event/input text) (handle-input text)
```

无论是 plain tuple 还是 `defenum` 生成的 tuple，优先使用原生 `match`；`tag-match` 主要保留给遗留代码或需要兼容旧写法的场景。

**Vector (`[]`)** - 可变、用于列表操作

```cirru.no-check
; 创建 vector
[] item1 item2 item3

; DOM 列表
div {} $ []
  button {} |Click
  span {} |Text
```

**常见错误：**

```cirru.no-check
; ❌ 错误：用 vector 传事件
send-event! $ [] :clipboard/read text
; 报错：tag-match expected tuple

; ✅ 正确：用 tuple
send-event! $ :: :clipboard/read text
```

### 类型标注与检查

Calcit 提供了静态类型分析系统，可以在预处理阶段发现潜在的类型错误。

#### 1. 顶层定义优先使用 `:schema`，局部函数继续使用 `hint-fn`

现在更推荐这样分工：

- 顶层 `defn` / `defmacro` 的参数、返回值、泛型信息，优先写到 `:schema`
- 局部 `fn` / 内部辅助函数，继续用 `hint-fn`
- `assert-type` 仍然可用，但更适合做函数体内的额外约束或中间值检查，而不是顶层定义的主标注方式

验证示例：

```cirru
let
    sum-items $ fn (items)
      foldl items 0 $ fn (acc item)
        hint-fn $ {}
          :args $ [] :number :number
          :return :number
        &+ acc item
  sum-items ([] 1 2 3)
```

#### 2. 返回类型标注

有两种方式标注函数返回类型：

- **紧凑模式（推荐）**：紧跟在参数列表后的类型标签。
- **正式模式**：局部 `fn` 使用 `hint-fn`（通常放在函数体开头）；顶层 `defn` / `defmacro` 使用 `:schema`。
  - 泛型变量：`hint-fn $ {} (:generics $ [] 'T 'S)`
  - 旧 clause 写法（如 `(hint-fn (return-type ...))` / `(generics ...)` / `(type-vars ...)`）已不再支持，会直接报错。

验证示例：

```cirru
let
    ; 紧凑模式
    add $ fn (a b) :number
      &+ a b
    ; 正式模式
    get-name $ fn (user)
      hint-fn $ {} (:args $ [] :dynamic) (:return :string)
      , |demo
    ; 泛型声明示例
    id $ fn (x)
      hint-fn $ {} (:generics $ [] 'T) (:args $ [] 'T) (:return 'T)
      x
  add 1 2
```

#### 3. 支持的类型标签

| 标签       | 说明              |
| ---------- | ----------------- |
| `:number`  | 数字              |
| `:string`  | 字符串            |
| `:bool`    | 布尔值            |
| `:symbol`  | 符号              |
| `:tag`     | 标签 (Keyword)    |
| `:list`    | 列表              |
| `:map`     | 哈希映射          |
| `:set`     | 集合              |
| `:tuple`   | Tuple             |
| `:fn`      | 函数              |
| `:dynamic` | 任意类型 (通配符) |

> 约定：动态类型标注统一使用 `:dynamic`，不再使用 `:any` 或 `nil` 作为 dynamic 的显式写法。

**高阶函数（HOF）回调类型检查：**

内置 HOF（`foldl`、`sort`、`filter`、`find`、`find-index`、`filter-not`、`mapcat`、`group-by` 等）的回调参数已强制要求 `:fn` 类型。传入非函数值（如数字、字符串）时会在预处理阶段触发类型警告：

```bash
# ❌ 错误：第三个参数应为函数，传了数字
cr eval 'foldl (list 1 2 3) 0 42'
# Type warning: expects :fn but got :number

# ✅ 正确
cr eval 'foldl (list 1 2 3) 0 &+'
```

#### 4. 复杂类型标注

- **可选类型**：`:: :optional :string` (可以是 string 或 nil)
- **变长参数**：在 Schema 中使用 `:rest :number` (参数列表剩余部分均为 number)
- **结构体/枚举**：使用 `defstruct` 或 `defenum` 定义的名字

验证示例 (使用 `let` 封装多表达式以支持 `cr eval` 验证)：

```cirru
let
    ; 可选参数
    greet $ fn (name)
      hint-fn $ {} (:args $ [] (:: :optional :string)) (:return :string)
      str "|Hello " (or name "|Guest")

    ; 变长参数
    sum $ fn (& xs)
      hint-fn $ {} (:rest :number) (:return :number)
      reduce xs 0 &+

    ; Record 约束 (使用 defstruct 定义结构体)
    User $ defstruct User (:name :string)
    get-name $ fn (u)
      hint-fn $ {} (:args $ [] (:: :record User)) (:return :string)
      get u :name
  println $ greet |Alice
  println $ sum 1 2 3
  println $ get-name (%{} User (:name |Bob))
```

**验证类型：** 运行或者编译时会先完成校验.

#### 5. Schema 与 `defn` / `defmacro` 一致性检查

如果定义带有 `:schema`，现在不仅 `cr analyze check-types` 会检查，普通运行路径也会在 **preprocess 阶段** 直接校验：

- `:: :fn` 必须对应 `defn`
- `:: :macro` 必须对应 `defmacro`
- `:args` 的必选参数个数必须和实际参数列表一致
- `:rest` 必须和代码里的 `&` rest 参数一致

这意味着下面几类命令都会在启动时直接失败，而不是等到 `analyze` 才发现：

- `cr <file>`
- `cr --check-only <file>`
- `cr <file> js`
- `yarn try-rs`

复杂但正确的示例（顶层用 `:schema`，局部函数用 `hint-fn`）：

```cirru.no-check
|join-str $ %{} :CodeEntry (:doc |)
  :code $ quote
    defn join-str (xs0 sep)
      apply-args (| xs0 true)
        defn %join-str (acc xs beginning?)
          hint-fn $ {}
            :args $ [] :string :list :bool
            :return :string
          list-match xs
            () acc
            (x0 xss)
              recur
                &str:concat
                  if beginning? acc $ &str:concat acc sep
                  , x0
                , xss false
  :examples $ []
  :schema $ :: :fn $ {} (:return :string)
    :args $ [] :list :string
```

这个例子里，schema 与代码是完全对齐的：

- `:: :fn` 对应 `defn`
- `:args` 里 2 个必选参数，对应 `(xs0 sep)`
- `:return :string` 对应整个 `join-str` 的返回值
- 内部辅助函数 `%join-str` 不是顶层定义，所以继续用 `hint-fn`

可以简单记忆为：**namespace 上的定义看 `:schema`，函数体内部的辅助函数看 `hint-fn`。**

推荐工作流：

```bash
# 先查看 calcit.core 里真实存在的 schema
cr query schema calcit.core/join-str

# 再仿照它给自己的定义写 schema
cr edit schema app.main/my-fn -e ':: :fn $ {} (:args $ [] :list :string) (:return :string)'

# 最后验证
cr --check-only
cr analyze check-types
```

实务上，`analyze check-types` 更适合做全量巡检；普通运行路径现在会做 fail-fast 阻断。

### 其他易错点

比较容易犯的错误：

- Calcit 中字符串通过前缀区分，`|` 和 `"` 开头表示字符串。`|x` 对应 JavaScript 字符串 `"x"`。产生 JSON 时注意不要重复包裹引号。
- Calcit 采用 Cirru 缩进语法，可以理解成去掉跨行括号改用缩进的 Lisp 变种。用 `cr cirru parse` 和 `cr cirru format` 互相转化试验。
- Calcit 跟 Clojure 在语义上比较像，但语法层面只用圆括号，不用方括号花括号。

---

## 开发调试

简单脚本可直接使用 `cr <filepath>` 执行（默认单次）。编译 JavaScript 用 `cr <filepath> js` 执行一次编译。
若需要监听模式，显式添加 `-w` / `--watch`（如 `cr -w <filepath>`、`cr <filepath> js -w`）。

Calcit snapshot 文件中 config 有 `init-fn` 和 `reload-fn` 配置：

- 初次启动调用 `init-fn`
- 每次修改代码后调用 `reload-fn`

**典型开发流程：**

```bash
# 1. 启动监听模式（用户自行使用）
cr -w        # 解释执行监听模式
cr js -w     # JS 编译监听模式
cr ir -w     # IR 生成监听模式

# 2. 修改代码后触发增量更新（详见"增量触发更新"章节）
cr edit inc --changed ns/def

# 3. 一次性执行/编译（用于简单脚本）
cr             # 执行一次
cr js          # 编译一次
cr ir          # 生成一次 IR
```

### 增量触发更新（推荐）⭐⭐⭐

当使用监听模式（`cr -w` / `cr js -w` / `cr ir -w`）开发时，推荐使用 `cr edit inc` 命令触发增量更新，而非全量重新编译/执行：

**工作流程：**

```bash
# 【终端 1】启动 watcher（监听模式）
cr -w        # 或 cr js -w / cr ir -w

# 【终端 2】修改代码后触发增量更新
# 修改定义
cr edit def app.core/my-fn -e 'defn my-fn (x) (+ x 1)'

# 触发增量更新
cr edit inc --changed app.core/my-fn

# 等待 ~300ms 后查看编译结果
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

**查看编译结果：**

```bash
cr query error  # 命令会显示详细的错误信息或成功状态
```

`cr query error` 只能告诉你最近一次 Calcit 语义链路里有没有报错，例如解析、预处理、运行期异常；它**不能**证明浏览器 CSS、HTML 属性值、业务数据内容或外部系统配置是“合理的”。像 `|max(...)` 被误写成 `"|max(...)` 这类在 Cirru 层面仍合法的字符串，就可能通过 `cr query error`，但在浏览器渲染阶段失效。

**何时使用全量操作：**

```bash
# 极少数情况：增量更新不符合预期时
cr js              # 重新编译 JavaScript
cr                 # 重新执行程序

# 或重启监听模式（Ctrl+C 停止后重启）
cr        # 或 cr js
```

**增量更新优势：** 快速反馈、精确控制变更范围、watcher 保持运行状态

---

## 文档支持

遇到疑问时使用：

- `cr docs search <keyword>` - 搜索 Calcit 教程内容
- `cr docs agents [<heading> ...] [--full]` - 读取 Agent 指南（优先本地缓存，按天自动刷新）
- `cr docs scopes` - 查看可查 scope
- `cr docs list [--module <name>]` - 查看当前 scope 的文档文件
- `cr docs sections <filename> [--module <name>]` - 查看章节标题
- `cr docs read <filename> [<heading> ...]` - 读取整份文档或按标题查看章节
- `cr docs read <filename> --full` - 直接读取整份文档内容
- `cr docs read-lines <filename> -s <start> -n <lines>` - 按行读取文档
- `cr docs remote-libs [search|readme|scan-md]` - 访问远程库 registry/README
- `cr query ns <ns>` - 查看命名空间说明和函数文档
- `cr query peek <ns/def>` - 快速查看定义签名
- `cr query def <ns/def>` - 读取完整语法树
- `cr query examples <ns/def>` - 查看示例代码
- `cr query find <name>` - 跨命名空间搜索符号
- `cr query usages <ns/def>` - 查找定义的使用位置
- `cr query search <pattern> [-f <ns/def>]` - 搜索叶子节点
- `cr query search-expr <pattern> [-f <ns/def>]` - 搜索结构表达式
- `cr query error` - 查看最近的错误堆栈（仅覆盖 Calcit 语义与运行链路，不覆盖 CSS/DOM/业务值合理性）

---

## 代码修改示例

### 添加新函数

```bash
# Cirru one liner
cr edit def app.core/multiply -e 'defn multiply (x y) (* x y)'
```

### 基本操作

```bash
# 添加新函数（命令会提示 Next steps）
cr edit def 'app.core/multiply' -e 'defn multiply (x y) (* x y)'

# 替换整个定义（推荐用 overwrite，避免依赖根路径替换）
cr edit def 'app.core/multiply' --overwrite -f /tmp/multiply.cirru

# 更新文档和示例
cr edit doc 'app.core/multiply' '乘法函数，返回两个数的积'
cr edit add-example 'app.core/multiply' -e 'multiply 5 6'

# 移动或重构定义
cr edit mv 'app.core/multiply' 'app.util/multiply-numbers'
```

### 修改定义工作流（命令会显示子节点索引和 Next steps）

```bash
# 1. 搜索定位
cr query search '<pattern>' -f 'ns/def'

# 2. 查看节点（输出会显示索引和操作提示）
cr tree show 'ns/def' -p '<path>'

# 3. 执行替换（会显示 diff 和验证命令）
cr tree replace 'ns/def' -p '<path>' --leaf -e '<value>'

# 4. 检查结果
cr query error
# 若改动涉及 CSS / DOM / 浏览器行为，继续做实际渲染验证，不要把 query error 当最终验收
# 添加命名空间（推荐：先创建空 ns，再逐条 add-import）
cr edit add-ns app.util
cr edit add-import app.util -e 'calcit.core :refer $ echo'

# 添加导入规则（单条）
cr edit add-import app.main -e 'app.util :refer $ helper'
# 覆盖已有同名 import
cr edit add-import app.main -e 'app.util :refer $ helper util-fn' -o

# 移除导入规则
cr edit rm-import app.main app.util

# 全量替换 imports（单条用 -e，多条用 -f 文件或 -j JSON）
cr edit imports app.main -e 'app.util :refer $ helper'          # 单条
cr edit imports app.main -f my-imports.cirru                    # 多条（每行一条规则）
cr edit imports app.main -j '[["app.lib",":as","lib"],["app.util",":refer",["helper"]]]'  # JSON

# 更新项目配置
cr config set init-fn app.main/main!
```

---

---

## 🔧 实战重构场景

以下是开发中最常见的局部修复和重构操作，帮助 Agent 快速找到对应命令。

### 提取子表达式为新定义（`edit split-def`）

**场景：** 函数体内某个嵌套子表达式太复杂，想拆成独立的命名定义。

```bash
# 1. 搜索并定位目标子表达式
cr query search-expr 'complex-call arg1' -f 'app.core/process-data'
# 输出示例：[3.2.1] in (let ((x ...)) ...)

# 2. 提取为新定义（原位置自动替换为新名字 extracted-calc）
cr edit split-def 'app.core/process-data' -p '3.2.1' -n extracted-calc

# 3. 查看结果
cr query def 'app.core/extracted-calc'   # 新定义
cr query def 'app.core/process-data'     # 原定义（原位已变成 extracted-calc）

# 4. 如需给新定义加函数签名（用 tree replace 重构根节点）
cr tree replace 'app.core/extracted-calc' -p '' -e 'defn extracted-calc (x) body-expr'
```

**注意：**`split-def` 仅创建新定义并替换引用，不会自动在其他 ns 添加 import。对外暴露时记得 `cr edit add-import`。

### 重命名定义（`edit rename`）

**场景：** 定义名字需要在同一命名空间内改名。

```bash
# 1. 确认有哪些地方引用到
cr query usages 'app.core/old-name'

# 2. 重命名（不允许覆盖已有定义）
cr edit rename 'app.core/old-name' 'new-name'

# 3. 批量更新所有引用（search 会自动提示批量命令）
cr query search 'old-name'   # 找到所有引用位置
cr tree replace-leaf 'app.core/caller-fn' --pattern 'old-name' -e 'new-name' --leaf
```

### 迁移定义到另一命名空间（`edit mv-def`）

**场景：** 某函数放错了命名空间，需要迁移。

```bash
# 移动定义
cr edit mv-def 'app.core/helper-fn' 'app.util/helper-fn'

# 在使用方添加 import
cr edit add-import 'app.main' -e 'app.util :refer $ helper-fn'

# 通知 watcher（热更新场景）
cr edit inc --removed 'app.core/helper-fn' --added 'app.util/helper-fn'
```

### 在定义内移动 / 复制 AST 节点（`edit mv` / `edit cp`）

**场景：** 函数体内某个子表达式需要移到另一位置，或复制用于多处。

```bash
# 定位节点
cr query search-expr 'process item' -f 'app.core/main-fn'
# 输出：[3,1,2]

# 移动（原位置消失）
cr edit mv 'app.core/main-fn' --from '3.1.2' -p '3.2' --at before

# 复制（原位置保留，新位置多一份）
cr edit cp 'app.core/main-fn' --from '3.1.2' -p '3.2' --at after
```

### 包裹 / 拆包 / 提升节点（`tree wrap` / `tree unwrap` / `tree raise`）

**场景：** 临时包裹一层 `println` 调试、反向拆掉包装层、或用子节点替换掉父节点。

```bash
# 包裹（wrap）：将节点包进新表达式，self = 原节点
cr tree wrap 'app.core/main-fn' -p '3.2' -e 'println self'

# 包裹成 let 绑定（self = 原表达式）
cr tree wrap 'app.core/main-fn' -p '3.2' -e 'let ((result self)) result'

# 拆包（unwrap）：删除该节点，所有子节点展开到原位置
cr tree unwrap 'app.core/main-fn' -p '3.2'

# 提升（raise）：用该子节点整体替换其父节点
# 场景：去掉 if 只保留 then 分支，或去掉 let 只保留最终返回值
cr tree raise 'app.core/main-fn' -p '3.2.1'
```

### 批量重命名局部变量（`tree replace-leaf` / `tree search-replace`）

**场景：** 某函数内某个局部变量名需要统一改掉。

```bash
# 若只有一处：内容定位直接替换（最安全 ⭐）
cr tree search-replace 'app.core/process' --pattern 'old-var' -e 'new-var' --leaf

# 若多处：一次性全部替换
cr tree replace-leaf 'app.core/process' --pattern 'old-var' -e 'new-var' --leaf
```

---

## ⚠️ 常见陷阱和最佳实践

### 1. 路径索引动态变化问题 ⭐⭐⭐

**核心原则：** 删除/插入会改变同级后续节点索引。

**批量修改策略：**

- **从后往前操作**（推荐）：先删大索引，再删小索引
- **单次操作后重新搜索**：每次修改立即用 `cr query search` 更新路径
- **整体重写**：优先用 `cr edit def --overwrite -f <file>`；`cr tree replace -p ''` 只保留给明确需要根节点级别改写的场景

命令会在路径错误时提示最长有效路径和可用子节点。

### 1.5 根路径整体替换的边界 ⭐⭐⭐

`cr tree replace -p ''` 在语义上确实是替换根节点，但在实际操作里，它更像“根 AST 节点替换”，而不是“整条定义安全重写”。当你需要完整替换一个定义体时：

- 更推荐 `cr edit def <ns/def> --overwrite -f <file>`
- 先在文件里组织完整定义，再一次性覆盖，验证也更直接
- 如果你已经用 `-p ''` 替换成功，仍应立刻执行 `cr query def <ns/def>` 或完整运行，确认写回后的定义结构符合预期

经验上，`-p ''` 更适合你已经非常确定根节点结构时的精细 AST 操作，不适合作为默认“全量改写定义”的模板。

### 2. 输入格式参数使用速查 ⭐⭐⭐

**参数混淆矩阵（已全面支持 `-e` 自动识别）：**

| 场景                | 示例用法                               | 解析结果                      | 说明                              |
| ------------------- | -------------------------------------- | ----------------------------- | --------------------------------- |
| **表达式 (Cirru)**  | `-e 'defn add (a b) (+ a b)'`          | `["defn", "add", ...]` (List) | 默认按 Cirru one-liner 解析       |
| **原子符号 (Leaf)** | `--leaf -e 'my-symbol'`                | `"my-symbol"` (Leaf)          | **推荐**，避免被包装成 list       |
| **字符串 (Leaf)**   | `--leaf -e '\|hello world'`            | `"hello world"` (Leaf)        | 符号前缀 `\|` 表示字符串          |
| **JSON 数组**       | `-e '["+", "x", "1"]'`                 | `["+", "x", "1"]` (List)      | **自动识别** (含 `[` 且有 `"`)    |
| **JSON 字符串**     | `-e '"my leaf"'`                       | `"my leaf"` (Leaf)            | **自动识别** (含引用的字符串)     |
| **内联 JSON**       | `-j '["defn", ...]'`                   | `["defn", ...]` (List)        | 显式按 JSON 解析，忽略 Cirru 规则 |
| **外部文件**        | `-f code.cirru` (或 `-f code.json -J`) | 根据文件内容解析              | `-J` 用于标记文件内是 JSON        |

**核心规则：**

1. **智能识别模式**：`-e / --code` 现在会自动识别 JSON。如果你传入 `["a"]` 或 `"a"`，它会直接按 JSON 处理，无需再额外加 `-J` 或 `-j`。
2. **强制 Leaf 模式**：如果你需要确保输入是一个叶子节点（符号或字符串），请在任何地方使用 `--leaf` 开关。它会将原始输入直接作为内容，不经过任何解析。
3. **显式 JSON 模式**：如果你想明确告诉工具“这段就是 JSON”，优先用 `-j '<json>'`。
4. **统一性**：`cr tree` 和 `cr edit` 的所有子命令（replace, def, insert 等）现在共享完全相同的输入解析逻辑。

**实战示例：**

```bash
# ✅ 替换表达式
cr tree replace app.main/fn -p '2' -e 'println |hello'

# ✅ 替换 leaf（推荐 --leaf）
cr tree replace app.main/fn -p '2.0' --leaf -e 'new-symbol'

# ✅ 替换字符串 leaf
cr tree replace app.main/fn -p '2.1' --leaf -e '|new text'

# ❌ 避免：用 -e 传单个 token（会变成 list）
cr tree replace app.main/fn -p '2.0' -e 'symbol'  # 结果：["symbol"]
```

### 3. Cirru 字符串和数据类型 ⭐⭐

**Cirru 字符串前缀：**

| Cirru 写法     | JSON 等价      | 使用场景     |
| -------------- | -------------- | ------------ |
| `\|hello`      | `"hello"`      | 推荐，简洁   |
| `"hello"`      | `"hello"`      | 也可以       |
| `\|a b c`      | `"a b c"`      | 包含空格     |
| `\|[tag] text` | `"[tag] text"` | 包含特殊字符 |

**不放心修改是否正确？** 每步后用 `tree show` 验证.

**Tuple vs Vector：**

```cirru.no-check
; ✅ Tuple - 用于事件、模式匹配
:: :clipboard/read text

; ✅ Vector - 用于 DOM 列表
[] (button) (div)

; ❌ 错误：用 vector 传事件
send-to-component! $ [] :clipboard/read text
; 报错：tag-match expected tuple

; ✅ 正确：用 tuple
send-to-component! $ :: :clipboard/read text
```

**记忆规则：**

- **`::` (tuple)**: 事件、模式匹配、不可变数据结构
- **`[]` (vector)**: DOM 元素列表、动态集合

### 4. 输入大小限制 ⭐⭐⭐

为了保证稳定性和处理速度，CLI 对单次输入的大小有限制。如果超过限制，系统会提示建议分段提交。

- **Cirru One-liner (`-e / --code`)**: 字数上限 **1000**。
- **JSON 格式 (`-j / --json`, `-J`, `-e`)**: 字数上限 **2000**。

**大资源处理建议：**
如果需要修改复杂的长函数，不要尝试一次性替换整个定义。应先构建主体结构，使用占位符，统一写成 `{{PLACEHOLDER_FEATURE}}` 这种花括号形式，并注意避免重复，然后通过 `cr tree search-replace` 或按路径的 `cr tree replace` 做精准的分段替换。

补充提示：现在 `cr query def` 和 `cr tree show` 遇到大表达式时会自动输出分片结果。`tree show` 默认只展开 ROOT 与一层 chunk；若需要继续查看 chunk 中的 chunk，可显式增加 `--chunk-expand-depth`。若你采用多阶段创建，建议从第一步就使用 `{{NAME}}` 风格占位符，这样后续在分片视图中更容易识别骨架、复制坐标并继续填充内容。

### 5. 命名空间操作陷阱 ⭐⭐⭐

**三个命令的 `-e` 期望格式完全不同，是最常见的混淆来源：**

| 命令                     | `-e` 期望内容                                                     | 错误用法                                               |
| ------------------------ | ----------------------------------------------------------------- | ------------------------------------------------------ |
| `add-ns <ns> -e ...`     | **完整 `ns` 表达式**：`ns my.ns $ :require ...`                   | ❌ 传 import 规则（静默成功但 ns 代码损坏）            |
| `imports <ns> -e ...`    | **单条 import 规则**（无 `:require` 前缀）：`src-ns :refer $ sym` | ❌ 带 `:require` 前缀（导致 `:require :require` 重复） |
| `add-import <ns> -e ...` | **单条 import 规则**（同上）：`src-ns :refer $ sym`               | 同 imports                                             |

**具体陷阱：**

❌ **陷阱1：`add-ns -e` 传了 import 规则而非完整 `ns` 表达式**

```bash
# ❌ 错误：ns 代码会变成 'respo.core :refer $ defcomp'（缺 ns 关键字！）
cr edit add-ns my.ns -e 'respo.core :refer $ defcomp'

# ✅ 正确：无代码时先建空 ns，再 add-import
cr edit add-ns my.ns
cr edit add-import my.ns -e 'respo.core :refer $ defcomp'

# ✅ 也正确：传完整 ns 表达式（名称必须与位置参数一致）
cr edit add-ns my.ns -e 'ns my.ns $ :require respo.core :refer $ defcomp'
```

❌ **陷阱2：`imports -e` 带了 `:require` 前缀**（现在会报错）

```bash
# ❌ 错误：现在会报错 "Do not include ':require' as a prefix"
cr edit imports my.ns -e ':require respo.core :refer $ sym'

# ✅ 正确：直接传规则，不加 :require
cr edit imports my.ns -e 'respo.core :refer $ sym'
```

❌ **陷阱3：`add-ns -e` 中 ns 名称与位置参数不一致**（现在会报错）

```bash
# ❌ 错误：现在会报错 "Namespace name mismatch"
cr edit add-ns my.ns -e 'ns wrong.ns $ :require ...'
```

❌ **陷阱4：想添加多条 imports 时用 `-e` 而非 `-f`**

```bash
# ❌ 无法在单个 -e 中写多条规则（会合并为一条）
cr edit imports my.ns -e 'respo.core :refer $ div\nrespo.util.format :refer $ hsl'

# ✅ 多条规则用文件（每行一条规则，无需 :require 前缀）
printf 'respo.core :refer $ div\nrespo.util.format :refer $ hsl\n' > /tmp/imports.cirru
cr edit imports my.ns -f /tmp/imports.cirru

# ✅ 或用 JSON 格式
cr edit imports my.ns -j '[["respo.core",":refer",["div"]],["respo.util.format",":refer",["hsl"]]]'

# ✅ 或逐条 add-import（推荐，更安全）
cr edit add-import my.ns -e 'respo.core :refer $ div'
cr edit add-import my.ns -e 'respo.util.format :refer $ hsl'
```

**最佳实践：优先用 `add-import`（更安全，带校验）：**

- `add-import` 会验证 source-ns 格式，有 `--overwrite` 保护
- `imports` 全量替换，一旦格式错误会覆盖所有 imports
- 只有需要完全重置所有 imports 时才用 `imports`

❌ **陷阱5：在 Cirru 源码中合并 `:as` 和 `:refer` 到同一条 import 规则**

Calcit 的 import 规则解析器只支持 **3 元素规则**（`ns :as alias` 或 `ns :refer (syms)`）。合并写法 `ns :as alias :refer (syms)` 不会报错，但 `:refer` 部分会被静默丢弃，导致符号无法解析。

```cirru.no-check
;; ❌ 错误：:refer 部分被静默丢弃，Op 无法被找到
ns app.main $ :require
  app.schema :as schema :refer $ Op

;; ✅ 正确：拆成两条独立规则
ns app.main $ :require
  app.schema :as schema
  app.schema :refer $ Op
```

对应 CLI 操作也需要分两次：

```bash
cr edit add-import app.main -e 'app.schema :as schema'
cr edit add-import app.main -e 'app.schema :refer $ Op'
```

### 6. 推荐工作流程

**基本流程（search 快速定位 ⭐⭐⭐）：**

```bash
# 1. 快速定位（比逐层导航快10倍）
cr query search 'target' -f 'ns/def'           # 或 search-expr 'fn (x)' 搜索结构

# 2. 执行修改（会显示 diff 和验证命令）
cr tree replace 'ns/def' -p '<path>' --leaf -e '<value>'

# 3. 增量更新（推荐）
cr edit inc --changed ns/def
# 等待 ~300ms 后检查
cr query error
```

**新手提示：**

- 不知道目标在哪？用 `search` 或 `search-expr` 快速找到所有匹配
- 想了解代码结构？用 `tree show` 逐层探索
- 需要批量重命名？搜索后按提示从大到小路径依次修改
- 不确定修改是否正确？每步后用 `tree show` 验证

### 7. Shell 特殊字符转义 ⭐⭐

Calcit 函数名中的 `?`, `->`, `!` 等字符在 bash/zsh 中有特殊含义，需要用单引号包裹：

```bash
# ❌ 错误
cr query def app.main/valid?
cr eval '-> x (+ 1) (* 2)'

# ✅ 正确
cr query def 'app.main/valid?'
cr eval 'thread-first x (+ 1) (* 2)'  # 用 thread-first 代替 ->
```

**建议：** 命令行中优先使用英文名称（`thread-first` 而非 `->`），更清晰且无需转义。

### 8. 多命令 `&&` 链式调用风险 ⭐⭐⭐

把多个 `cr tree replace`、`cr edit def -e ...` 或其他带内联代码的命令用 `&&` 串起来，在 bash/zsh 中风险很高：

- 只要某一段 `-e` 内容里出现未正确转义的引号，shell 就会进入“继续等待补全输入”的状态，看起来像终端卡死
- 前一条命令如果已经改写了内容，后一条命令即使没执行，你也可能以为整批操作已完成

更稳妥的做法：

- 批量修改时逐条执行
- 多行或含引号内容改用 `-f <file>`
- 需要批量脚本化时，放到独立 shell script，并先用最小样例验证 quoting

---

## 🔄 完整功能开发示例

以下展示从零开始添加新函数的完整流程，是最常见的日常开发场景。

### 步骤 1：确认目标命名空间和现有代码

```bash
# 查看命名空间列表
cr query ns

# 查看某个 ns 已有的定义
cr query defs app.util

# 快速了解某个定义（不展开完整代码）
cr query peek 'app.util/format-date'

# 如有疑问，读取完整代码
cr query def 'app.util/format-date'
```

### 步骤 2：用 eval 快速验证写法

在真正写入项目前，先用 `cr eval` 验证逻辑思路：

```bash
# 验证基础函数调用
cr eval 'string->number |123'

# 验证带 let 的表达式
cr eval 'let ((x 10) (y 20)) (+ x y)'

# 验证列表操作
cr eval 'let ((xs (list 1 2 3))) (map xs (fn (x) (* x 2)))'

# 加载项目依赖模块后测试
cr eval --dep calcit.std 'str/split |hello world | '
```

> 💡 `cr eval` 有类型警告时会失败退出——正好可以提前发现用法错误。

### 步骤 3：添加新定义

```bash
# 在已有命名空间中添加新函数
cr edit def 'app.util/calculate-discount' -e 'defn calculate-discount (price rate) (* price (- 1 rate))'

# 验证定义写入成功
cr query def 'app.util/calculate-discount'
```

### 步骤 4：在调用方添加 import 并使用

```bash
# 查看调用方当前 imports
cr query ns app.core

# 添加 import（首选 add-import，更安全）
cr edit add-import 'app.core' -e 'app.util :refer $ calculate-discount'

# 在函数体中使用新定义（先定位插入位置）
cr query search 'total-price' -f 'app.core/checkout'
# 输出：[3.2.1] in (let ((total-price ...)) ...)

# 修改调用
cr tree replace 'app.core/checkout' -p '3.2.1' -e 'calculate-discount total-price 0.1'
```

### 步骤 5：触发热更新并验证

```bash
# 推送增量更新（触发 watcher 热加载）
cr edit inc --changed 'app.util/calculate-discount'
cr edit inc --changed 'app.core/checkout'

# 等待 ~300ms 后检查是否有错误
cr query error

# 如无错误，用 --check-only 整体验证
cr --check-only
```

如果这次改动涉及样式、浏览器属性、字符串模板或外部接口，`cr query error` 和 `cr --check-only` 通过后，仍要继续做目标环境里的真实验收。

### 常见失误快速修复

```bash
# 忘记 import → unknown symbol
cr edit add-import 'app.core' -e 'app.util :refer $ calculate-discount'

# 定义名拼写错误 → 重命名
cr edit rename 'app.util/calculte-discount' 'calculate-discount'

# 函数参数顺序传错 → 定位并修改调用
cr query search 'calculate-discount' -f 'app.core/checkout'
cr tree replace 'app.core/checkout' -p '3.2.1' --leaf -e 'calculate-discount'
```

---

## 💡 Calcit vs Clojure 关键差异

**语法层面：**

- **只用圆括号**：Calcit 的 Cirru 语法不使用方括号 `[]` 和花括号 `{}`，统一用缩进表达结构
- **函数前缀**：Calcit 用 `&` 区分内置函数（`&+`、`&str`）和用户定义函数

**集合函数参数顺序（易错 ⭐⭐⭐）：**

- **Calcit**: 集合在**第一位** → `map data fn` 或 `-> data (map fn)`
- **Clojure**: 函数在第一位 → `map fn data` 或 `->> data $ map fn`
- **症状**：`unknown data for foldl-shortcut` 报错
- **原因**：误用 `->>` 或参数顺序错误

**其他差异：**

- **宏系统**：Calcit 更简洁，缺少 Clojure 的 reader macro（如 `#()`）
- **数据类型**：Calcit 的 Tuple (`::`) 和 Vector (`[]`) 有特定用途（见"Cirru 字符串和数据类型"）

---

## 常见错误排查

### 快速诊断流程

当 watcher 提示有错误或行为异常时，按以下顺序排查：

```bash
# 1. 查看最新错误堆栈（首选）
cr query error
# 输出示例：
#   Error in app.core/process-data
#   CalcitErr: unknown symbol: proess-item   ← 拼写错误
#   at app.core/render → app.core/process-data → ...

# 2. 用 --check-only 快速全量验证（不执行程序）
cr --check-only

# 3. 用 cr eval 隔离验证单个函数写法
cr eval 'let ((x 1)) (+ x 2)'
```

### 错误信息对照表

| 错误信息                                         | 原因                                              | 解决方法                                                                 |
| ------------------------------------------------ | ------------------------------------------------- | ------------------------------------------------------------------------ |
| `Path index X out of bounds`                     | 路径索引已过期（操作后变化）                      | 重新运行 `cr query search` 获取最新路径                                  |
| `tag-match expected tuple`                       | 传入 vector 而非 tuple                            | 改用 `::` 语法，如 `:: :event-name data`                                 |
| `unknown symbol: xxx`                            | 符号未定义或未 import                             | `cr query find xxx` 确认位置，`cr edit add-import` 引入                  |
| `expects pairs in list for let`                  | `let` 绑定语法错误                                | 改为 `let ((x val)) body`（双层括号）                                    |
| `cannot be used as operator`                     | 末尾符号被当作函数调用                            | 改用 `, acc` 前缀传递值，或用函数包裹                                    |
| `unknown data for foldl-shortcut`                | 参数顺序错误（Calcit vs Clojure 差异）            | Calcit 集合在第一位：`map data fn`                                       |
| `Do not include ':require' as prefix`            | `cr edit imports` 格式错误                        | 去掉 `:require` 前缀，直接传 `src-ns :refer $ sym`                       |
| `Namespace name mismatch`                        | `add-ns -e` 名称不一致                            | ns 表达式名称必须与位置参数完全一致                                      |
| 字符串被拆分成多个 token                         | 没有用 `\|` 或 `"` 包裹                           | 使用 `\|complete string` 或 `"complete string`                           |
| `unexpected format`                              | Cirru 语法错误                                    | 用 `cr cirru parse '<code>'` 验证语法                                    |
| `Type warning` 导致 eval 失败                    | 类型不匹配（阻断执行）                            | 优先检查 `:schema` / `hint-fn` 的参数标注；局部值再用 `assert-type` 复核 |
| `schema mismatch while preprocessing definition` | `:schema` 与 `defn` / `defmacro` / 参数个数不一致 | 修正 `:kind`、`:args`、`:rest`，或让代码定义与 schema 保持一致           |
| `cr query error` 无报错但页面仍异常              | 问题不在 Calcit 语义链路，而在 CSS/DOM/业务值     | 到真实运行环境核对渲染结果、属性值和外部依赖，而不是只看 `query error`   |

### 调试常用命令

```bash
# 查看完整错误栈（最详细）
cr query error

# 检查某个定义的代码和内容
cr query def 'ns/def'
cr tree show 'ns/def'

# 验证 Cirru 语法
cr cirru parse 'defn add (a b) (+ a b)'

# 快速测试某个想法（不影响项目代码）
cr eval 'range 5'
cr eval 'let ((xs (list 1 2 3))) (map xs number->string)'

# 检查定义是否存在
cr query find 'my-function'
cr query defs 'my.namespace'
```

> 💡 **错误文件备份**：`.calcit-error.cirru` 会保存最近一次的完整错误堆栈（包含 chain 信息），比 `cr query error` 更完整。直接用 `cat .calcit-error.cirru` 读取，或 `cr query error`（从此文件读取并格式化输出）。
