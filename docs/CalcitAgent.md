# Calcit 编程 Agent 指南

本文档为 AI Agent 提供 Calcit 项目的操作指南。

## 🚀 快速开始（新 LLM 必读）

**核心原则：用命令行工具（不要直接编辑文件），用 search 定位（比逐层导航快 10 倍）**

### 标准流程

```bash
# 搜索 → 修改 → 验证
cr query search 'symbol' -f 'ns/def'                      # 1. 定位（输出：[3,2,1] in ...）
cr tree replace 'ns/def' -p '3,2,1' --leaf -e 'new'     # 2. 修改
cr tree show 'ns/def' -p '3,2,1'                          # 3. 验证（可选）
```

### 三种搜索方式

```bash
cr query search 'target' -f 'ns/def'                      # 搜索符号/字符串
cr query search-expr 'fn (x)' -f 'ns/def' -l              # 搜索代码结构
cr tree replace-leaf 'ns/def' --pattern 'old' -e 'new' --leaf  # 批量替换叶子节点
```

### 效率对比

| 操作       | 传统方法                | search 方法         | 效率     |
| ---------- | ----------------------- | ------------------- | -------- |
| 定位符号   | 逐层 `tree show` 10+ 步 | `query search` 1 步 | **10倍** |
| 查找表达式 | 手动遍历代码            | `search-expr` 1 步  | **10倍** |
| 批量重命名 | 手动找每处              | 自动列出所有位置    | **5倍**  |

---

## ⚠️ 重要警告：禁止直接修改的文件

以下文件**严格禁止使用文本替换或直接编辑**：

- **`calcit.cirru`** - 这是 calcit-editor 结构化编辑器的专用格式，包含完整的编辑器元数据
- **`compact.cirru`** - 这是 Calcit 程序的紧凑快照格式，必须使用 `cr edit` 相关命令进行修改

这两个文件的格式对空格和结构极其敏感，直接文本修改会破坏文件结构。请使用下面文档中的 CLI 命令进行代码查询和修改。

## Calcit 与 Cirru 的关系

- **Calcit** 是编程语言本身（一门类似 Clojure 的函数式编程语言）
- **Cirru** 是语法格式（缩进风格的 S-expression，类似去掉括号改用缩进的 Lisp）
- **关系**：Calcit 代码使用 Cirru 语法书写和存储

**具体体现：**

- `compact.cirru` 和 `calcit.cirru` 是用 Cirru 格式存储的 Calcit 程序
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

- `cr` 或 `cr compact.cirru` - 代码解释执行，默认读取 config 执行 init-fn 定义的入口（默认单次执行后退出）
- `cr -w` 或 `cr --watch` - 解释执行监听模式（显式启用监听）
- `cr compact.cirru js` - 编译生成 JavaScript 代码（默认单次编译）
- `cr compact.cirru js -w` / `cr compact.cirru js --watch` - JS 监听编译模式
- `cr compact.cirru ir` - 生成 program-ir.cirru（默认单次生成）
- `cr compact.cirru ir -w` / `cr compact.cirru ir --watch` - IR 监听生成模式
- `cr -1 <filepath>` - 执行一次然后退出（兼容参数，当前默认行为已是 once）
- `cr --check-only` - 仅检查代码正确性，不执行程序
  - 对 init_fn 和 reload_fn 进行预处理验证
  - 输出：预处理进度、warnings、检查耗时
  - 用于 CI/CD 或快速验证代码修改
- `cr js -1` - 检查代码正确性，生成 JavaScript(兼容参数，默认已是单次)
- `cr js --check-only` - 检查代码正确性，不生成 JavaScript
- `cr eval '<code>' [--dep <module>...]` - 执行一段 Calcit 代码片段，用于快速验证写法
  - `--dep` 参数可以加载 `~/.config/calcit/modules/` 中的模块（直接使用模块名）
  - 示例：`cr eval 'echo 1' --dep calcit.std`
  - 可多次使用 `--dep` 加载多个模块

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
- `cr query modules` - 列出项目依赖的模块（来自 compact.cirru 配置）

**渐进式代码探索（Progressive Disclosure）：**

- `cr query peek <namespace/definition>` - 查看定义签名（参数、文档、表达式数量），不返回完整实现体
  - 输出：Doc、Form 类型、参数列表、Body 表达式数量、首个表达式预览、Examples 数量
  - 用于快速了解函数接口，减少 token 消耗
- `cr query def <namespace/definition> [-j]` - 读取定义的完整 Cirru 代码
  - 默认输出：Doc、Examples 数量、Cirru 格式代码
  - `-j` / `--json`：同时输出 JSON 格式（用于程序化处理）
  - 推荐：LLM 直接读取 Cirru 格式即可，通常不需要 JSON
- `cr query examples <namespace/definition>` - 读取定义的示例代码
  - 输出：每个 example 的 Cirru 格式和 JSON 格式

**符号搜索与引用分析：**

- `cr query find <symbol> [--deps] [-f] [-n <limit>]` - 跨命名空间搜索符号
  - 默认精确匹配：返回定义位置 + 所有引用位置（带上下文预览）
  - `-f` / `--fuzzy`：模糊搜索，匹配 "namespace/definition" 格式的路径
  - `-n <limit>`：限制模糊搜索结果数量（默认 20）
  - `--deps`：包含核心命名空间（calcit.\* 开头）
- `cr query usages <namespace/definition> [--deps]` - 查找定义的所有使用位置
  - 返回：引用该定义的所有位置（带上下文预览）
  - 用于理解代码影响范围，重构前的影响分析

**代码模式搜索（快速定位 ⭐⭐⭐）：**

- `cr query search <pattern> [-f <filter>] [-l]` - 搜索叶子节点（符号/字符串），比逐层导航快 10 倍
  - `-f <filter>` - 过滤到特定命名空间或定义
  - `-l / --loose`：宽松匹配，包含模式
  - `-d <max-depth>`：限制搜索深度
  - `-p <start-path>`：从指定路径开始搜索（如 `"3,2,1"`）
  - 返回：完整路径 + 父级上下文，多个匹配时自动显示批量替换命令
  - 示例：
    - `cr query search 'println' -f app.main/main!` - 精确搜索
    - `cr query search 'comp-' -f app.ui/layout -l` - 模糊搜索（所有 comp- 开头）
    - `cr query search 'task-id' -f app.comp/render` - 返回所有匹配位置并自动排序

**高级结构搜索（搜索代码结构 ⭐⭐⭐）：**

- `cr query search-expr <pattern> [-f <filter>] [-l] [-j]` - 搜索结构表达式（List）
  - `-l / --loose`：宽松匹配，从头部开始的前缀匹配（嵌套表达式也支持前缀）
  - `-j / --json`：将模式解析为 JSON 数组
  - 示例：
    - `cr query search-expr 'fn (x)' -f app.main/process -l` - 查找函数定义
    - `cr query search-expr '>> state task-id' -l` - 查找状态访问（匹配 `>> state task-id ...` 或 `>> state`）
    - `cr query search-expr 'dispatch! (:: :states)' -l` - 匹配 `dispatch! (:: :states data)` 类型的表达式
    - `cr query search-expr 'memof1-call-by' -l` - 查找记忆化调用

**搜索结果格式：** `[索引1,索引2,...] in 父级上下文`，可配合 `cr tree show <ns/def> -p '<path>'` 查看节点。**修改代码时优先用 search 命令，比逐层导航快 10 倍。**

### LLM 辅助：动态方法提示

- `&methods-of` - 返回某个值在运行时可用的方法名列表（字符串，包含前导点）
  - 用法：`&methods-of value`
  - 返回：`[] |.foo |.bar ...`

- `&inspect-methods` - 打印某个值在运行时可用的方法与 impl 记录来源（不改变原值）
  - 用法：`&inspect-methods value "|optional note"`
  - 用途：调试动态分派/traits override 链，适合临时插入 pipeline

- `&impl:origin` - 读取 impl record 的 trait 来源（返回 trait 值或 nil）
  - 用法：`&impl:origin impl`
  - 用途：调试/断言 impl 与 trait 的关联关系（配合 `&tuple:impls` 或 `&methods-of` 使用）

- `&trait-call` - 显式调用某个 trait 的方法实现（同名方法消歧/绕开 `.method` 分派）
  - 用法：`&trait-call Trait :method receiver & args`
  - 说明：会按当前 value 的 impl precedence 扫描，但只匹配“属于该 trait”的 impl 记录；若 trait 定义了 default 实现则会回退调用
  - 前置条件：建议用 `defimpl` 创建 impl（impl record 会保存 trait origin，供 `&trait-call` 定位）

### 文档子命令 (`cr docs`)

查询 Calcit 语言文档（guidebook）：

- `cr docs search <keyword> [-c <num>] [-f <filename>]` - 按关键词搜索文档内容
  - `-c <num>` - 显示匹配行的上下文行数（默认 5）
  - `-f <filename>` - 按文件名过滤搜索结果
  - 输出：匹配行及其上下文，带行号和高亮
  - 示例：`cr docs search 'macro' -c 10` 或 `cr docs search 'defn' -f macros.md`

- `cr docs read <filename> [-s <start>] [-n <lines>]` - 阅读指定文档
  - `-s <start>` - 起始行号（默认 0）
  - `-n <lines>` - 读取行数（默认 80）
  - 输出：文档内容、当前范围、是否有更多内容
  - 示例：`cr docs read macros.md` 或 `cr docs read intro.md -s 20 -n 30`

- `cr docs list` - 列出所有可用文档

### Cirru 语法工具 (`cr cirru`)

用于 Cirru 语法和 JSON 之间的转换：

- `cr cirru parse '<cirru_code>'` - 解析 Cirru 代码为 JSON
- `cr cirru format '<json>'` - 格式化 JSON 为 Cirru 代码
- `cr cirru parse-edn '<edn>'` - 解析 Cirru EDN 为 JSON
- `cr cirru show-guide` - 显示 Cirru 语法指南（帮助 LLM 生成正确的 Cirru 代码）

**⚠️ 重要：生成 Cirru 代码前请先阅读语法指南**

运行 `cr cirru show-guide` 获取完整的 Cirru 语法说明，包括：

- `$` 操作符（单节点展开）
- `|` 前缀（字符串字面量）, 这个是 Cirru 特殊的地方, 而不是直接用引号包裹
- `,` 操作符（注释标记）
- `~` 和 `~@`（宏展开）
- 常见错误和避免方法

### 库管理 (`cr libs`)

查询和了解 Calcit 官方库：

- `cr libs` - 列出所有官方库
- `cr libs search <keyword>` - 按关键词搜索库（搜索名称、描述、分类）
- `cr libs readme <package> [-f <file>]` - 查看库的文档
  - 优先从本地 `~/.config/calcit/modules/<package>` 读取
  - 本地不存在时从 GitHub 仓库获取
  - `-f` 参数可指定其他文档文件（如 `-f Skills.md`）
  - 默认读取 `README.md`
- `cr libs scan-md <module>` - 扫描本地模块目录下的所有 `.md` 文件
  - 递归扫描子目录
  - 显示相对路径列表
- `caps` - 安装/更新依赖

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

- 路径格式：逗号分隔的索引（如 `"3,2,1"`），空字符串 `""` 表示根节点
- 每个命令都有 `--help` 查看详细参数
- 命令执行后会显示 "Next steps" 提示下一步操作

**主要操作：**

- `cr tree show <ns/def> -p '<path>' [-j]` - 查看节点
  - 默认输出：节点类型、Cirru 预览、子节点索引列表、操作提示
  - `-j` / `--json`：同时输出 JSON 格式（用于程序化处理）
  - 推荐：直接查看 Cirru 格式即可，通常不需要 JSON
- `cr tree replace` - 替换节点
- `cr tree replace-leaf` - 查找并替换所有匹配的 leaf 节点（无需指定路径）
  - `--pattern <pattern>` - 要搜索的模式（精确匹配 leaf 节点）
  - 使用 `-e, -f, -j` 等通用参数提供替换内容
  - 自动遍历整个定义，一次性替换所有匹配项
  - 示例：`cr tree replace-leaf 'ns/def' --pattern 'old-name' -e 'new-name' --leaf`
- `cr tree target-replace` - 基于内容的唯一替换（无需指定路径，更安全 ⭐⭐⭐）
  - `--pattern <pattern>` - 要搜索的模式（精确匹配 leaf 节点）
  - 使用 `-e, -f, -j` 等通用参数提供替换内容
  - 逻辑：自动查找叶子节点，若唯一则替换；若不唯一则报错并列出所有位置及修改命令建议。
- `cr tree delete` - 删除节点
- `cr tree insert-before/after` - 插入相邻节点
- `cr tree insert-child/append-child` - 插入子节点
- `cr tree swap-next/prev` - 交换相邻节点
- `cr tree wrap` - 用新结构包装节点

**输入方式（通用）：**

- `-e '<code>'` - 内联代码（自动识别 Cirru/JSON）
- `--leaf` - 强制作为 leaf 节点（符号或字符串）
- `-j '<json>'` / `-f <file>`

多行或者带特殊符号的表达式, 一可以在 `.calcit-snippets/` 创建临时文件, 然后用 `cr cirru parse` 验证语法, 最后用 `-f <file>` 提交, 从而减少错误率. 复杂表达式建议分段, 然后搭配 `cr tree target-replace` 命令来完成多阶段提交.

**推荐工作流（高效定位 ⭐⭐⭐）：**

```bash
# ===== 方案 A：单点修改（精确定位） =====

# 1. 快速定位目标节点（一步到位）
cr query search 'target-symbol' -f namespace/def
# 输出：[3,2,5,1] in (fn (x) target-symbol ...)

# 2. 直接修改（路径已知）
cr tree replace namespace/def -p '3,2,5,1' --leaf -e 'new-symbol'

# 3. 验证结果（可选）
cr tree show namespace/def -p '3,2,5,1'


# ===== 方案 B：批量重命名（多处修改） =====

# 1. 搜索所有匹配位置
cr query search 'old-name' -f namespace/def
# 自动显示：4 处匹配，已按路径从大到小排序
# [3,2,5,8] [3,2,5,2] [3,1,0] [2,1]

# 2. 按提示从后往前修改（避免路径变化）
cr tree replace namespace/def -p '3,2,5,8' --leaf -e 'new-name'
cr tree replace namespace/def -p '3,2,5,2' --leaf -e 'new-name'
# ... 继续按序修改

# 或：一次性替换所有匹配项
cr tree replace-leaf namespace/def --pattern 'old-name' -e 'new-name' --leaf


# ===== 方案 C：基于内容的半自动替换（最推荐 ⭐⭐⭐） =====

# 1. 尝试基于叶子节点内容直接替换
cr tree target-replace namespace/def --pattern 'old-symbol' -e 'new-symbol' --leaf

# 2. 如果存在多个匹配，命令会报错并给出详细指引（包含具体路径的 replace 命令建议）
# 如果确定要全部替换，可改用 tree replace-leaf


# ===== 方案 D：结构搜索（查找表达式） =====

# 1. 搜索包含特定模式的表达式
cr query search-expr "fn (task)" -f namespace/def -l
# 输出：[3,2,2,5,2,4,1] in (map $ fn (task) ...)

# 2. 查看完整结构（可选）
cr tree show namespace/def -p '3,2,2,5,2,4,1'

# 3. 修改整个表达式或子节点
cr tree replace namespace/def -p '3,2,2,5,2,4,1,2' -e 'let ((x 1)) (+ x task)'
```

**关键技巧：**

- **优先使用 `search` 系列命令**：比逐层导航快 10+ 倍，一步直达目标
- **路径格式**：`"3,2,1"` 表示第3个子节点 → 第2个子节点 → 第1个子节点
- **批量修改自动提示**：搜索找到多处时，自动显示路径排序和批量替换命令
- **路径动态变化**：删除/插入后，同级后续索引会变化，按提示从后往前操作
- 所有命令都会显示 Next steps 和操作提示

**结构化变更示例：**

这些高级操作允许你在修改时引用原始节点及其内部结构：

- **包裹节点**（使用 `cr tree wrap` 或 `cr tree replace` 的 `--refer-original`）：

  ```bash
  # 将路径 "3,2" 的节点包裹在 println 中
  cr tree wrap ns/def -p '3,2' -e 'println $$$$' --refer-original '$$$$'
  ```

- **重构并复用原子节点**（使用 `--refer-inner-branch`）：
  - 假设原节点是 `+ 1 2` (路径 "3,1")，其子节点索引 1 是 `1`，索引 2 是 `2`
  - 将其重构为 `* 2 10`：

  ```bash
  cr tree replace ns/def -p '3,1' -e '(* #### 10)' --refer-inner-branch '2' --refer-inner-placeholder '####'
  ```

- **多处重用原始节点**：
  ```bash
  # 将节点 x 变为 (+ x x)
  cr tree replace ns/def -p '2' -e '(+ $ $)' --refer-original '$'
  ```
  详细参数和示例使用 `cr tree <command> --help` 查看。

### 复杂表达式分段组装策略 (Incremental Assembly) ⭐⭐⭐

当需要构造非常复杂的嵌套结构（例如递归循环、多级 `let` 或 `if`）时，直接通过 `-e` 传入单行 Cirru 代码容易遇到 shell 转义、括号对齐或长度限制等问题。推荐使用**分段占位组装**策略：

1. **确立骨架**：先替换目标节点为一个带有占位符的简单 JSON 结构。

   ```bash
   cr tree replace ns/def -p '4,0' -j '["let", [["x", "1"]], "BODY"]'
   ```

2. **定位占位符**：使用 `tree show` 确认占位符的具体路径。

   ```bash
   cr tree show ns/def -p '4,0'
   # 输出显示 "BODY" 在索引 2，即路径 [4,0,2]
   ```

3. **填充内容**：针对占位符路径进行下一层的精细替换。

   ```bash
   cr tree replace ns/def -p '4,0,2' -j '["if", ["=", "x", "1"], "TRUE_BRANCH", "FALSE_BRANCH"]'
   ```

4. **递归迭代**：重复上述步骤直到所有占位符（`TRUE_BRANCH`, `FALSE_BRANCH` 等）都被替换为最终逻辑。

**优势：**

- **精确性**：使用 JSON 格式 (`-j`) 可以完全避免 Cirru 缩进或括号解析的歧义。
- **低风险**：每次只修改一小部分，出错时容易通过 `tree show` 快速定位。
- **绕过限制**：解决某些终端对超长命令行参数的限制。

### 代码编辑 (`cr edit`)

直接编辑 compact.cirru 项目代码，支持两种输入方式：

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

- `cr edit def <namespace/definition>` - 添加新定义（若已存在会报错，需用 `cr tree replace` 修改）
- `cr edit mv <source> <target>` - 移动定义到另一个命名空间或重命名
- `cr edit rm-def <namespace/definition>` - 删除定义
- `cr edit doc <namespace/definition> '<doc>'` - 更新定义的文档
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

**模块和配置：**

- `cr edit add-module <module-path>` - 添加模块依赖
- `cr edit rm-module <module-path>` - 删除模块依赖
- `cr edit config <key> <value>` - 设置配置（key: init-fn, reload-fn, version）

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

```cirru
; 创建 tuple
:: :event/type data

; 模式匹配
tag-match event
  (:event/click data) (handle-click data)
  (:event/input text) (handle-input text)
```

**Vector (`[]`)** - 可变、用于列表操作

```cirru
; 创建 vector
[] item1 item2 item3

; DOM 列表
div {} $ []
  button {} |Click
  span {} |Text
```

**常见错误：**

```cirru
; ❌ 错误：用 vector 传事件
send-event! $ [] :clipboard/read text
; 报错：tag-match expected tuple

; ✅ 正确：用 tuple
send-event! $ :: :clipboard/read text
```

### 类型标注与检查

Calcit 提供了静态类型分析系统，可以在预处理阶段发现潜在的类型错误。

#### 1. 参数类型标注 (`assert-type`)

使用 `assert-type` 标注参数类型。它可以出现在函数体内的任何位置。

验证示例：

```cirru
let
    calculate-total $ fn (items discount)
      assert-type items :list
      assert-type discount :number
      let
          sum $ foldl items 0 &+
        * sum $ - 1 discount
  calculate-total ([] 1 2 3) 0.1
```

#### 2. 返回类型标注

有两种方式标注函数返回类型：

- **紧凑模式（推荐）**：紧跟在参数列表后的类型标签。
- **正式模式**：使用 `hint-fn`（通常放在函数体开头）。
  - 泛型变量：`hint-fn (:: :generics 'T 'S)`

验证示例：

```cirru
let
    ; 紧凑模式
    add $ fn (a b) :number
      &+ a b
    ; 正式模式
    get-name $ fn (user)
      hint-fn $ return-type :string
      |demo
    ; 泛型声明示例
    id $ fn (x)
      hint-fn (:: :generics 'T)
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

#### 4. 复杂类型标注

- **可选类型**：`:: :optional :string` (可以是 string 或 nil)
- **变长参数**：`:: :& :number` (参数列表剩余部分均为 number)
- **结构体/枚举**：使用 `defstruct` 或 `defenum` 定义的名字

验证示例 (使用 `let` 封装多表达式以支持 `cr eval` 验证)：

```cirru
let
    ; 可选参数
    greet $ fn (name)
      assert-type name $ :: :optional :string
      str "|Hello " (or name "|Guest")

    ; 变长参数
    sum $ fn (& xs)
      assert-type xs $ :: :& :number
      reduce xs 0 &+

    ; Record 约束 (使用 defstruct 定义结构体)
    User $ defstruct User (:name :string)
    get-name $ fn (u)
      assert-type u User
      get u :name
  println $ greet |Alice
  println $ sum 1 2 3
  println $ get-name (%{} User (:name |Bob))
```

**验证类型：** 运行或者编译时会先完成校验.

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

**何时使用全量操作：**

```bash
# 极少数情况：增量更新不符合预期时
cr -1 js           # 重新编译 JavaScript
cr -1              # 重新执行程序

# 或重启监听模式（Ctrl+C 停止后重启）
cr        # 或 cr js
```

**增量更新优势：** 快速反馈、精确控制变更范围、watcher 保持运行状态

---

## 文档支持

遇到疑问时使用：

- `cr docs search <keyword>` - 搜索 Calcit 教程内容
- `cr docs read <filename>` - 阅读完整文档
- `cr docs list` - 查看所有可用文档
- `cr query ns <ns>` - 查看命名空间说明和函数文档
- `cr query peek <ns/def>` - 快速查看定义签名
- `cr query def <ns/def>` - 读取完整语法树
- `cr query examples <ns/def>` - 查看示例代码
- `cr query find <name>` - 跨命名空间搜索符号
- `cr query usages <ns/def>` - 查找定义的使用位置
- `cr query search <pattern> [-f <ns/def>]` - 搜索叶子节点
- `cr query search-expr <pattern> [-f <ns/def>]` - 搜索结构表达式
- `cr query error` - 查看最近的错误堆栈

---

## 代码修改示例

**添加新函数：**

````bash
# Cirru one liner
cr edit def app.core/multiply -e 'defn multiply (x y) (* x y)'
# 基本操作：**

```bash
# 添加新函数（命令会提示 Next steps）
cr edit def 'app.core/multiply' -e 'defn multiply (x y) (* x y)'

# 替换整个定义（-p '' 表示根路径）
cr tree replace 'app.core/multiply' -p '' -e 'defn multiply (x y z) (* x y z)'

# 更新文档和示例
cr edit doc 'app.core/multiply' '乘法函数，返回两个数的积'
cr edit add-example 'app.core/multiply' -e 'multiply 5 6'

# 移动或重构定义
cr edit mv 'app.core/multiply' 'app.util/multiply-numbers'
````

**修改定义工作流（命令会显示子节点索引和 Next steps）：**

```bash
# 1. 搜索定位
cr query search '<pattern>' -f 'ns/def' -l

# 2. 查看节点（输出会显示索引和操作提示）
cr tree show 'ns/def' -p '<path>'

# 3. 执行替换（会显示 diff 和验证命令）
cr tree replace 'ns/def' -p '<path>' --leaf -e '<value>'

# 4. 检查结果
cr query error
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
cr edit config init-fn app.main/main!
```

---

## ⚠️ 常见陷阱和最佳实践

### 1. 路径索引动态变化问题 ⭐⭐⭐

**核心原则：** 删除/插入会改变同级后续节点索引。

**批量修改策略：**

- **从后往前操作**（推荐）：先删大索引，再删小索引
- **单次操作后重新搜索**：每次修改立即用 `cr query search` 更新路径
- **整体重写**：用 `cr tree replace -p ''` 替换整个定义

命令会在路径错误时提示最长有效路径和可用子节点。

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
cr tree replace app.main/fn -p '2,0' --leaf -e 'new-symbol'

# ✅ 替换字符串 leaf
cr tree replace app.main/fn -p '2,1' --leaf -e '|new text'

# ❌ 避免：用 -e 传单个 token（会变成 list）
cr tree replace app.main/fn -p '2,0' -e 'symbol'  # 结果：["symbol"]
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

```cirru
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
如果需要修改复杂的长函数，不要尝试一次性替换整个定义。应先构建主体结构，使用占位符（如 `?PLACEHOLDER_FEATURE`, 注意避免重复），然后通过 `cr tree target-replace` 进行精准的分段替换.

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

### 6. 推荐工作流程

**基本流程（search 快速定位 ⭐⭐⭐）：**

```bash
# 1. 快速定位（比逐层导航快10倍）
cr query search 'target' -f 'ns/def'           # 或 search-expr 'fn (x)' -l 搜索结构

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

| 错误信息                     | 原因                    | 解决方法                          |
| ---------------------------- | ----------------------- | --------------------------------- |
| `Path index X out of bounds` | 路径已过期              | 重新运行 `cr query search`        |
| `tag-match expected tuple`   | 传入 vector 而非 tuple  | 改用 `::`                         |
| 字符串被拆分                 | 没有用 `\|` 或 `"` 包裹 | 使用 `\|complete string`          |
| `unexpected format`          | 语法错误                | 用 `cr cirru parse '<code>'` 验证 |

**调试命令：** `cr query error`（会显示详细的错误堆栈和提示）
