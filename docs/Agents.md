# Calcit 编程 Agent 指南

本文档为 AI Agent 提供 Calcit 项目的操作指南。

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

- `cr` 或 `cr compact.cirru` - 代码解释执行，默认读取 config 执行 init-fn 定义的入口
- `cr compact.cirru js` - 编译生成 JavaScript 代码
- `cr -1 <filepath>` - 执行一次然后退出（不进入监听模式）
- `cr --check-only` - 仅检查代码正确性，不执行程序
  - 对 init_fn 和 reload_fn 进行预处理验证
  - 输出：预处理进度、warnings、检查耗时
  - 用于 CI/CD 或快速验证代码修改
- `cr js -1` - 检查代码正确性，生成 JavaScript(不进入监听模式)
- `cr js --check-only` - 检查代码正确性，不生成 JavaScript
- `cr eval '<code>'` - 执行一段 Calcit 代码片段，用于快速验证写法

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
- `cr query modules` - 列出项目模块

**渐进式代码探索（Progressive Disclosure）：**

- `cr query peek <namespace/definition>` - 查看定义签名（参数、文档、表达式数量），不返回完整实现体
  - 输出：Doc、Form 类型、参数列表、Body 表达式数量、首个表达式预览、Examples 数量
  - 用于快速了解函数接口，减少 token 消耗
- `cr query def <namespace/definition>` - 读取定义的完整语法树（JSON 格式）
  - 同时显示 Doc 和 Examples 的完整内容
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

**代码模式搜索：**

- `cr query search <pattern> [-f <namespace/definition>] [-l] [-d <depth>]` - 搜索叶子节点（字符串）

  - `<pattern>` - 位置参数，要搜索的字符串模式
  - `-f` / `--filter` - 过滤到特定命名空间或定义（可选）
  - `-l` / `--loose`：宽松匹配，包含模式（匹配所有包含该模式的叶子节点）
  - `-d <depth>`：限制搜索深度（0 = 无限制）
  - 返回：匹配节点的完整路径 + 父级上下文预览
  - 示例：
    - `cr query search "println" -f app.main/main -l` - 在 main 函数中搜索包含 "println" 的节点
    - `cr query search "div"` - 全局精确搜索 "div"

- `cr query search-pattern <pattern> [-f <namespace/definition>] [-l] [-j] [-d <depth>]` - 搜索结构模式
  - `<pattern>` - 位置参数，Cirru one-liner 或 JSON 数组模式
  - `-f` / `--filter` - 过滤到特定命名空间或定义（可选）
  - `-l` / `--loose`：宽松匹配，查找包含连续子序列的结构
  - `-j` / `--json`：将模式解析为 JSON 数组而非 Cirru
  - 返回：匹配节点的路径 + 父级上下文
  - 示例：
    - `cr query search-pattern "(+ a b)" -f app.util/add` - 查找精确表达式
    - `cr query search-pattern '["defn"]' -f app.main/main -j -l` - 查找所有函数定义

**搜索结果格式：**

- 输出格式：`[路径] in 父级上下文`
- 路径格式：`[索引1,索引2,...]` 表示从根节点到匹配节点的路径
- 可配合 `cr tree show <target> -p "<path>"` 查看具体节点内容

### 文档子命令 (`cr docs`)

查询 Calcit 语言文档（guidebook）：

- `cr docs search <keyword> [-c <num>] [-f <filename>]` - 按关键词搜索文档内容

  - `-c <num>` - 显示匹配行的上下文行数（默认 5）
  - `-f <filename>` - 按文件名过滤搜索结果
  - 输出：匹配行及其上下文，带行号和高亮
  - 示例：`cr docs search "macro" -c 10` 或 `cr docs search "defn" -f macros.md`

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

### 精细代码树操作 (`cr tree`)

⚠️ **关键警告：路径索引动态变化**

删除或插入节点后，同级后续节点的索引会自动改变。**必须从后往前操作**或**每次修改后重新搜索路径**。详见 [常见陷阱 #1](#1-路径索引动态变化问题-)。

提供对 AST 节点的低级精确操作，适用于需要精细控制的场景：

**可用操作：**

- `cr tree show <namespace/definition> -p <path>` - 查看指定路径的节点

  - `-p <path>` - 节点路径，用逗号分隔索引（如 `"3,2,1"`），空字符串 `""` 表示根节点
  - `-d <depth>` - 限制显示深度（0=无限，默认 2）
  - **输出格式**：Cirru 缩进格式的代码树，`[索引]` 标注每个子节点位置
  - **使用技巧**：
    - 先用 `-d 1` 查看顶层结构，再逐层深入
    - 路径中的索引对应输出中的 `[0]`, `[1]`, `[2]` 等标记
    - 配合 `cr query search` 快速定位目标节点

- `cr tree replace <namespace/definition> -p <path>` - 替换指定路径的节点

  - **输入方式**（按推荐优先级）：

  1.  `-e '<expr>'` - **最推荐**。支持 Cirru one-liner，也支持自动识别 JSON (如 `-e '["a"]'`)。
  2.  `-j '<json>'` - 显式传入内联 JSON 字符串。
  3.  `-f <file>` - 从文件读取（默认 Cirru，加 `-J` 读 JSON）。
  4.  `-s` - 从 stdin 读取（默认 Cirru，加 `-J` 读 JSON）。

  - **特殊参数**：

    - `--leaf` - 配合 `-e`/`-f`/`-s` 将输入视为 leaf 节点
      - 符号：`--leaf -e 'my-var'`
      - 字符串：`--leaf -e '|text'`（实测最快捷，示例：`cr tree replace respo.app.core/new-fn -p "3,1" --leaf -e '|hello'`）
    - `--refer-original <name>` - 在新代码中引用原节点（高级用法）
    - `--refer-inner-branch <path>` - 引用原节点的内部分支
    - `--refer-inner-placeholder <name>` - 内部分支的占位符名

  - **快速决策**：
    - 替换表达式 → 用 `-e 'cirru one-liner'`
    - 替换单个值 → 用 `--leaf -e '<symbol 或 |string>'`（避免被包装成 list）

- `cr tree delete <namespace/definition> -p <path>` - 删除指定路径的节点

  - `-p <path>` - 要删除的节点路径（完整路径，含最后索引）
  - ⚠️ **副作用**：删除后，同级后续节点的索引会前移
  - **示例**：删除 `[3,2,1]` 后，原 `[3,2,2]` 变成 `[3,2,1]`

- `cr tree insert-before <namespace/definition> -p <path>` - 在指定位置前插入节点

  - `-p <path>` - 参考节点的路径（在此节点之前插入）
  - 输入方式同 `replace`（`-e`, `-j`, `-f`, `-s`）
  - **结果**：新节点占据当前索引，原节点及后续节点索引 +1

- `cr tree insert-after <namespace/definition> -p <path>` - 在指定位置后插入节点

  - `-p <path>` - 参考节点的路径（在此节点之后插入）
  - 输入方式同 `replace`
  - **结果**：新节点占据下一个索引，原节点索引不变

- `cr tree insert-child <namespace/definition> -p <path>` - 插入为第一个子节点

  - `-p <path>` - 父节点的路径
  - **结果**：新节点成为 `<path>,0`，原有子节点索引全部 +1

- `cr tree append-child <namespace/definition> -p <path>` - 追加为最后一个子节点

  - `-p <path>` - 父节点的路径
  - **结果**：新节点成为最后一个子节点，不影响其他节点索引
  - **推荐**：批量插入时优先用此命令（索引稳定）

- `cr tree swap-next <namespace/definition> -p <path>` - 与下一个兄弟节点交换位置

  - `-p <path>` - 当前节点路径
  - **结果**：当前节点索引 +1，下一个节点索引 -1
  - **应用场景**：调整参数顺序、移动代码位置

- `cr tree swap-prev <namespace/definition> -p <path>` - 与上一个兄弟节点交换位置

  - `-p <path>` - 当前节点路径
  - **结果**：当前节点索引 -1，上一个节点索引 +1

- `cr tree wrap <namespace/definition> -p <path>` - 用新结构包装节点
  - `-p <path>` - 要包装的节点路径
  - 输入方式同 `replace`，需使用 `--refer-original <name>` 引用原节点
  - **示例**：将 `x` 包装成 `(+ x 1)`
    ```bash
    cr tree wrap app.main/fn -p "2,0" -e '+ $original 1' --refer-original original
    ```

**实战示例：**

```bash
# 场景 1：查看函数结构（渐进式）
cr tree show app.main/main! -p "" -d 1      # 先看顶层
cr tree show app.main/main! -p "2" -d 2     # 深入第 3 个子节点
cr tree show app.main/main! -p "2,1,0"      # 查看具体节点

# 场景 2：替换符号 / 字符串（推荐用 --leaf）
cr tree replace app.main/add -p "2,0" --leaf -e '*'      # 将 + 改成 *
cr tree replace app.main/greet -p "1" --leaf -e '|Hello' # 替换字符串 leaf

# 场景 3：替换表达式
cr tree replace app.main/main! -p "2" -e 'println |modified'

# 场景 4：删除节点（注意索引变化）
cr query search "old-fn" -f app.main/main!  # 找到 [3,2,5]
cr tree delete app.main/main! -p "3,2,5"

# 场景 5：批量追加（安全方法）
cr tree append-child app.main/init! -p "2" -e 'step-1'
cr tree append-child app.main/init! -p "2" -e 'step-2'  # 索引稳定
cr tree append-child app.main/init! -p "2" -e 'step-3'

# 场景 6：包装节点（添加函数调用）
cr tree wrap app.main/fn -p "3,1" -e 'str $old' --refer-original old
# 将 x 变成 (str x)

# 场景 7：配合 search 定位后替换
cr query search ".show" -f app.comp.home/comp-box -l
# 输出：[3,2,1,3,1,3,2,1,2,0] in .show confirm-plugin ...
cr tree show app.comp.home/comp-box -p "3,2,1,3,1,3,2,1,2,0"  # 确认
cr tree replace app.comp.home/comp-box -p "3,2,1,3,1,3,2,1,2,0" --leaf -e '|.visible'

# demos/compact.cirru 实操（验证最新 CLI 输出）
# 目标：把 `respo.app.core/new-fn` 中的 `|hello` 暂时改成其他文案，再恢复
cr query search "println" -f respo.app.core/new-fn -l
# → 找到路径 [3,0] / [3,1]
cr tree show respo.app.core/new-fn -p "3,1" -d 2
# → CLI 会显示 Location/Type、JSON、下一步提示
cr tree replace respo.app.core/new-fn -p "3,1" --leaf -e '|hello from tree demo'
# → 输出 `Preview: replace ...`、Before/After diff、Verify 提示
cr tree replace respo.app.core/new-fn -p "3,1" --leaf -e '|hello'
# → 同样会给出 diff，确保 demos 恢复原状
# 以上流程适合在 demos/compact.cirru 做沙盒操作，熟悉路径与 diff 提示
```

**⚠️ 安全操作流程（必读）**

**单次修改标准流程：**

```bash
# 【步骤 1】模糊搜索快速定位
cr query search "target" -f app.core/my-fn -l
# 输出：[3,2,1,5] in target parent-context ...

# 【步骤 2】逐层确认路径正确（避免操作错误节点）
cr tree show app.core/my-fn -p "" -d 1        # 查看顶层，确认 [3] 存在
cr tree show app.core/my-fn -p "3,2" -d 2     # 查看上级，确认 [3,2,1] 存在
cr tree show app.core/my-fn -p "3,2,1,5"      # 查看目标节点内容

# 【步骤 3】执行修改
cr tree replace app.core/my-fn -p "3,2,1,5" --leaf -e '|new-value'

# 【步骤 4】验证修改结果
cr tree show app.core/my-fn -p "3,2,1,5"      # 确认内容已更新
cr --check-only                                # 语法检查
```

**批量修改策略：**

```bash
# ❌ 错误示范：连续删除（索引会变化）
cr tree delete app.main/fn -p "3,2,1"
cr tree delete app.main/fn -p "3,2,2"  # 错误！路径已失效

# ✅ 正确方法 A：从后往前操作
cr query search "pattern" -f app.main/fn  # 记录所有路径
cr tree delete app.main/fn -p "3,2,3"     # 先删除索引大的
cr tree delete app.main/fn -p "3,2,2"
cr tree delete app.main/fn -p "3,2,1"     # 最后删除索引小的

# ✅ 正确方法 B：单次操作后立即重新搜索
cr tree delete app.main/fn -p "3,2,1"
cr query search "pattern" -f app.main/fn  # 重新获取路径
cr tree delete app.main/fn -p "<新路径>"

# ✅ 正确方法 C：整体重写定义
cr query def app.main/fn > fn.json
# 编辑 fn.json
cr edit def app.main/fn -f fn.json -J
```

### 代码编辑 (`cr edit`)

直接编辑 compact.cirru 项目代码，支持三种输入方式：

- `--file <path>` 或 `-f <path>` - 从文件读取（默认 Cirru 格式，使用 `-J` 指定 JSON）
- `--json <string>` 或 `-j <string>` - 内联 JSON 字符串
- `--stdin` 或 `-s` - 从标准输入读取（默认 Cirru 格式，使用 `-J` 指定 JSON）

额外支持“内联代码”参数：

- `--code <text>` 或 `-e <text>`：直接在命令行里传入一段代码。
  - 默认按 **Cirru 单行表达式（one-liner）** 解析。
  - 如果输入“看起来像 JSON”（例如 `-e '"abc"'`，或 `-e '["a"]'` 这类 `[...]` 且包含 `"`），则会按 JSON 解析。
  - ⚠️ 当输入看起来像 JSON 但 JSON 不合法时，会直接报错（不会回退当成 Cirru one-liner）。

对 `--file/--stdin` 输入，还支持以下“格式开关”（与 `-J/--json-input` 类似）：

- `--leaf`：把输入当成 **leaf 节点**，直接使用 Cirru 符号或 `|text` 字符串，无需 JSON 引号。
  - 传入符号：`-e 'my-symbol'`
  - 传入字符串：加 Cirru 字符串前缀 `|` 或 `"`，例如 `-e '|my string'` 或 `-e '"my string'`

⚠️ 注意：这些开关彼此互斥（一次只用一个）。

**推荐简化规则（命令行更好写）：**

- **JSON（单行）**：优先用 `-j '<json>'` 或 `-e '<json>'`（不需要 `-J`）。
- **Cirru 单行表达式**：用 `-e '<expr>'`（`-e` 默认按 one-liner 解析）。
- **Cirru 多行缩进**：用 `-f file.cirru` 或 `-s`（stdin）。
- `-J/--json-input` 主要用于 **file/stdin** 读入 JSON（如 `-f code.json -J` 或 `-s -J`）。

补充：`-e/--code` 只有在 `[...]` 内部包含 `"` 时才会自动按 JSON 解析（例如 `-e '["a"]'`）。
像 `-e '[]'` / `-e '[ ]'` 会默认按 Cirru one-liner 处理；如果你需要“空 JSON 数组”，用显式 JSON：`-j '[]'`。

如果你想在命令行里明确“这段就是 JSON”，请用 `-j '<json>'`（`-J` 是给 file/stdin 用的）。

**定义操作：**

- `cr edit def <namespace/definition>` - 添加或更新定义
- `cr edit rm-def <namespace/definition>` - 删除定义
- `cr edit doc <namespace/definition> '<doc>'` - 更新定义的文档
- `cr edit examples <namespace/definition>` - 设置定义的示例代码（批量替换）
- `cr edit add-example <namespace/definition>` - 添加单个示例
- `cr edit rm-example <namespace/definition> <index>` - 删除指定索引的示例（0-based）

**命名空间操作：**

- `cr edit add-ns <namespace>` - 添加命名空间
- `cr edit rm-ns <namespace>` - 删除命名空间
- `cr edit imports <namespace>` - 更新导入规则（全量替换）
- `cr edit add-import <namespace>` - 添加单个 import 规则
- `cr edit rm-import <namespace> <source_ns>` - 移除指定来源的 import 规则
- `cr edit ns-doc <namespace> '<doc>'` - 更新命名空间文档

**模块和配置：**

- `cr edit add-module <module-path>` - 添加模块依赖
- `cr edit rm-module <module-path>` - 删除模块依赖
- `cr edit config <key> <value>` - 设置配置（key: init-fn, reload-fn, version）

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

### 其他易错点

比较容易犯的错误：

- Calcit 中字符串通过前缀区分，`|` 和 `"` 开头表示字符串。`|x` 对应 JavaScript 字符串 `"x"`。产生 JSON 时注意不要重复包裹引号。
- Calcit 采用 Cirru 缩进语法，可以理解成去掉跨行括号改用缩进的 Lisp 变种。用 `cr cirru parse` 和 `cr cirru format` 互相转化试验。
- Calcit 跟 Clojure 在语义上比较像，但语法层面只用圆括号，不用方括号花括号。

---

## 开发调试

简单脚本用 `cr -1 <filepath>` 直接执行。编译 JavaScript 用 `cr -1 <filepath> js` 执行一次编译。

Calcit snapshot 文件中 config 有 `init-fn` 和 `reload-fn` 配置：

- 初次启动调用 `init-fn`
- 每次修改代码后调用 `reload-fn`

**典型开发流程：**

```bash
# 1. 检查代码正确性
cr --check-only

# 2. 执行程序（一次性）
cr -1

# 3. 编译 JavaScript（一次性）
cr -1 js

# 4. 进入监听模式开发
cr        # 解释执行模式
cr js     # JS 编译模式
```

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
- `cr query search <ns/def> -p <pattern>` - 搜索叶子节点
- `cr query search-pattern <ns/def> -p <pattern>` - 搜索结构模式
- `cr query error` - 查看最近的错误堆栈

---

## 代码修改示例

**添加新函数：**

```bash
# Cirru one liner
cr edit def app.core/multiply -e 'defn multiply (x y) (* x y)'
# or JSON
cr edit def app.core/multiply -j '["defn", "multiply", ["x", "y"], ["*", "x", "y"]]'
```

**更新文档和示例：**

```bash
# 更新文档
cr edit doc app.core/multiply '乘法函数，返回两个数的积'

# 设置示例
cr edit examples app.core/multiply -j '[["multiply", "3", "4"]]'

# 添加示例
cr edit add-example app.core/multiply -e 'multiply 5 6'

# 删除示例
cr edit rm-example app.core/multiply 1
```

**局部修改（推荐流程）：**

```bash
# 1. 读取完整定义
cr query def app.core/add-numbers

# 2. 多次查看节点确认目标坐标
cr tree show app.core/add-numbers -p "" -d 1
cr tree show app.core/add-numbers -p "2" -d 1
cr tree show app.core/add-numbers -p "2,0"

# 3. 执行替换
cr tree replace app.core/add-numbers -p "2,0" -e '*'

# 4. 验证
cr tree show app.core/add-numbers -p "2"
```

**命名空间增量操作：**

```bash
# 添加命名空间
cr edit add-ns app.util

# 添加导入规则
cr edit add-import app.main -e 'app.util :refer $ helper'

# 移除导入规则
cr edit rm-import app.main app.util

# 更新项目配置
cr edit config init-fn app.main/main!
```

**更新命名空间导入（全量替换）：**

```bash
cr edit imports app.main -j '[["app.lib", ":as", "lib"], ["app.util", ":refer", ["helper"]]]'
```

---

## ⚠️ 常见陷阱和最佳实践

### 1. 路径索引动态变化问题 ⭐⭐⭐

**核心问题：** 删除或插入节点后，同级后续节点的索引会自动改变，导致之前查询到的路径立即失效。

**典型错误：**

```bash
# 假设搜索到两个节点：[3,2,1,2] 和 [3,2,1,3]
cr tree delete app.main/fn -p "3,2,1,2"
# 此时原来的 [3,2,1,3] 已变成 [3,2,1,2]
cr tree delete app.main/fn -p "3,2,1,3"  # ❌ 路径已失效，报错！
```

**解决方案：**

✅ **从后往前操作** - 先操作索引大的节点：

```bash
# 正确顺序：从后往前删除
cr tree delete app.main/fn -p "3,2,1,3"
cr tree delete app.main/fn -p "3,2,1,2"
cr tree delete app.main/fn -p "3,2,1,1"
```

✅ **单次操作后立即重新搜索：**

```bash
cr tree delete app.main/fn -p "3,2,1,2"
# 立即重新搜索获取更新后的路径
cr query search "target" -f app.main/fn
cr tree delete app.main/fn -p "<新路径>"
```

✅ **批量操作前一次性规划：**

```bash
# 1. 先收集所有需要修改的路径
cr query search "old-pattern" -f app.main/fn > paths.txt
# 2. 手动排序后从后往前执行
# 3. 或考虑用 cr edit def 整体重写定义
```

⚠️ **避免：** 基于单次搜索结果连续修改多个节点。

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
cr tree replace app.main/fn -p "2" -e 'println |hello'

# ✅ 替换 leaf（推荐 --leaf）
cr tree replace app.main/fn -p "2,0" --leaf -e 'new-symbol'

# ✅ 替换字符串 leaf
cr tree replace app.main/fn -p "2,1" --leaf -e '|new text'

# ❌ 避免：用 -e 传单个 token（会变成 list）
cr tree replace app.main/fn -p "2,0" -e 'symbol'  # 结果：["symbol"]
```

### 3. Cirru 字符串和数据类型 ⭐⭐

**Cirru 字符串前缀：**

| Cirru 写法     | JSON 等价      | 使用场景     |
| -------------- | -------------- | ------------ |
| `\|hello`      | `"hello"`      | 推荐，简洁   |
| `"hello"`      | `"hello"`      | 也可以       |
| `\|a b c`      | `"a b c"`      | 包含空格     |
| `\|[tag] text` | `"[tag] text"` | 包含特殊字符 |

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

### 4. 搜索结果路径的正确使用 ⭐

**搜索输出格式：**

```bash
cr query search ".show" -f app.comp.home/comp-box
# 输出：[3,2,1,3,1,3,2,1,2,0] in .show confirm-plugin d! (fn () ...)
#       ^^^^^^^^^^^^^^^^^^^^^^ 这是 .show 节点本身的路径
#                              父级路径是 [3,2,1,3,1,3,2,1,2]
```

**使用建议：**

- **查看节点**：用返回的完整路径
  ```bash
  cr tree show app.comp.home/comp-box -p "3,2,1,3,1,3,2,1,2,0"
  ```
- **替换节点**：用父级路径（去掉最后一个索引）
  ```bash
  cr tree replace app.comp.home/comp-box -p "3,2,1,3,1,3,2,1,2" -e 'new-expr'
  ```
- **插入/删除**：根据操作类型选择路径

  ```bash
  # 删除节点本身：用完整路径
  cr tree delete app.comp.home/comp-box -p "3,2,1,3,1,3,2,1,2,0"

  # 在节点后插入：用完整路径
  cr tree insert-after app.comp.home/comp-box -p "3,2,1,3,1,3,2,1,2,0" -e 'new-item'
  ```

### 5. 推荐工作流程

**精细代码修改流程：**

```bash
# 【步骤 1】模糊搜索定位大致位置
cr query search "target-keyword" -f namespace/definition -l

# 【步骤 2】逐层确认精确路径（多次运行，逐步深入）
cr tree show namespace/definition -p "" -d 1
cr tree show namespace/definition -p "3" -d 2
cr tree show namespace/definition -p "3,2,1" -d 2

# 【步骤 3】执行修改（单次操作）
cr tree replace namespace/definition -p "3,2,1" -e 'new-code'

# 【步骤 4】验证结果
cr tree show namespace/definition -p "3,2,1"
cr --check-only  # 语法检查
```

**批量修改策略：**

```bash
# 方案 A：从后往前操作
# 1. 收集所有路径并排序
cr query search "pattern" -f app.main/fn
# 记录：[2,1,3], [2,1,2], [2,1,1]

# 2. 从大到小执行
cr tree delete app.main/fn -p "2,1,3"
cr tree delete app.main/fn -p "2,1,2"
cr tree delete app.main/fn -p "2,1,1"

# 方案 B：整体重写
cr query def app.main/fn > fn.json
# 编辑 fn.json
cr edit def app.main/fn -f fn.json -J
```

---

## 常见错误排查

| 错误信息                            | 原因                                | 解决方法                                        |
| ----------------------------------- | ----------------------------------- | ----------------------------------------------- |
| `Path index X out of bounds`        | 路径已过期，节点被删除或索引改变    | 重新运行 `cr query search` 获取最新路径         |
| `tag-match expected tuple, got ...` | 传入 vector `[]` 而非 tuple `::`    | 改用 `::` 构造数据结构                          |
| 字符串被拆分成多个 token            | Cirru 字符串没有用 `\|` 或 `"` 包裹 | 使用 `\|complete string` 或 `"complete string"` |
| Cirru one-liner 变成 list           | `-e` 单个 token 会被包装成 list     | 用 `-j '"token"'` 传入 leaf 节点                |
| `unexpected format in expression`   | JSON 格式错误或 Cirru 语法错误      | 用 `cr cirru parse '<code>'` 验证语法           |

**调试技巧：**

```bash
# 1. 验证 Cirru 语法
cr cirru parse 'fn (x) (+ x 1)'

# 2. 验证 JSON 格式
echo '["fn", ["x"], ["+", "x", "1"]]' | jq .

# 3. 检查整体语法
cr --check-only

# 4. 查看错误堆栈
cr query error
```
