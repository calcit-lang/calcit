# Calcit 编程 Agent 指南

本文档为 AI Agent 提供 Calcit 项目的操作指南。

## ⚠️ 重要警告：禁止直接修改的文件

以下文件**严格禁止使用文本替换或直接编辑**：

- **`calcit.cirru`** - 这是 calcit-editor 结构化编辑器的专用格式，包含完整的编辑器元数据
- **`compact.cirru`** - 这是 Calcit 程序的紧凑快照格式，必须使用 `cr edit` 相关命令进行修改

这两个文件的格式对空格和结构极其敏感，直接文本修改会破坏文件结构。请使用下面文档中的 CLI 命令进行代码查询和修改。

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
- `cr js --check-only` - 检查代码正确性，不生成 JavaScript
- `cr eval '<code>'` - 执行一段 Calcit 代码片段，用于快速验证写法

### 查询子命令 (`cr query`)

这些命令用于查询项目信息：

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

- `cr query search <namespace/definition> -p <pattern> [-l] [-d <depth>]` - 搜索叶子节点（字符串）

  - 默认：精确匹配字符串（`-p "div"` 只匹配 `"div"`）
  - `-l` / `--loose`：宽松匹配，包含模式（`-p "di"` 匹配所有包含 "di" 的叶子节点）
  - `-d <depth>`：限制搜索深度（0 = 无限制）
  - 返回：匹配节点的完整路径 + 父级上下文预览
  - 示例：`cr query search app.main/main -p "println" -l`

- `cr query search-pattern <namespace/definition> -p <pattern> [-l] [-j] [-d <depth>]` - 搜索结构模式
  - 模式格式：Cirru one-liner 或 JSON 数组（使用 `-j` 标志）
  - 默认：精确结构匹配（整个结构完全相同）
  - `-l` / `--loose`：宽松匹配，查找包含连续子序列的结构
    - 例如：`-p '["defn", "add"]' -j -l` 匹配任何包含连续 `["defn", "add"]` 的列表
  - `-j` / `--json`：将模式解析为 JSON 数组而非 Cirru
  - 返回：匹配节点的路径 + 父级上下文
  - 示例：
    - `cr query search-pattern app.util/add -p "(+ a b)"` - 查找精确表达式
    - `cr query search-pattern app.main/main -p '["defn"]' -j -l` - 查找所有函数定义

**搜索结果格式：**

- 输出格式：`[路径] in 父级上下文`
- 路径格式：`[索引1,索引2,...]` 表示从根节点到匹配节点的路径
- 可配合 `cr tree show <target> -p "<path>"` 查看具体节点内容

### 文档子命令 (`cr docs`)

查询 Calcit 语言文档：

- `cr docs api <keyword>` - 搜索 API 文档（也可用 -t tag 按标签搜索）
- `cr docs ref <keyword>` - 搜索教程/指南文档
- `cr docs list-api` - 列出所有 API 文档主题
- `cr docs list-guide` - 列出所有教程文档主题

### Cirru 语法工具 (`cr cirru`)

用于 Cirru 语法和 JSON 之间的转换：

- `cr cirru parse '<cirru_code>'` - 解析 Cirru 代码为 JSON
- `cr cirru format '<json>'` - 格式化 JSON 为 Cirru 代码
- `cr cirru parse-edn '<edn>'` - 解析 Cirru EDN 为 JSON
- `cr cirru show-guide` - 显示 Cirru 语法指南（帮助 LLM 生成正确的 Cirru 代码）

**⚠️ 重要：生成 Cirru 代码前请先阅读语法指南**

运行 `cr cirru show-guide` 获取完整的 Cirru 语法说明，包括：

- `$` 操作符（单节点展开）
- `|` 前缀（字符串字面量）
- `,` 操作符（表达式终止符）
- `~` 和 `~@`（宏展开）
- 常见错误和避免方法

### 库管理 (`cr libs`)

查询和了解 Calcit 官方库：

- `cr libs` - 列出所有官方库
- `cr libs search <keyword>` - 按关键词搜索库（搜索名称、描述、分类）
- `cr libs readme <package>` - 查看指定库的 README 文档（从 GitHub 获取）
- `caps` - 安装/更新依赖

### 代码分析 (`cr analyze`)

分析代码结构和调用关系：

**调用图分析：**

- `cr analyze call-graph` - 分析从入口点开始的调用图结构
  - `--root <ns/def>` - 指定分析入口（默认使用 init_fn）
  - `--ns-prefix <prefix>` - 只显示指定命名空间前缀的定义
  - `--include-core` - 包含 calcit.core 核心库函数
  - `--max-depth <n>` - 限制遍历深度（0=无限）
  - `--show-unused` - 显示未使用的定义
  - `--format text|json` - 输出格式

**调用次数统计：**

- `cr analyze count-calls` - 统计每个定义的调用次数
  - `--root <ns/def>` - 指定分析入口（默认使用 init_fn）
  - `--ns-prefix <prefix>` - 只显示指定命名空间前缀的定义
  - `--include-core` - 包含 calcit.core 核心库函数
  - `--format text|json` - 输出格式
  - `--sort count|name` - 按调用次数（默认，降序）或名称排序

**使用示例：**

```bash
# 分析整个项目的调用图
cr analyze call-graph

# 分析特定入口点
cr analyze call-graph --root app.main/main!

# 只看特定命名空间前缀
cr analyze call-graph --ns-prefix app.
```

### 精细代码树操作 (`cr tree`)

提供对 AST 节点的低级精确操作，适用于需要精细控制的场景：

**可用操作：**

- `cr tree show <namespace/definition> -p <path>` - 查看指定路径的节点

  - `-d <depth>` - 限制显示深度（0=无限，默认 2）

- `cr tree replace <namespace/definition> -p <path>` - 替换指定路径的节点

  - `-e <code>` - 内联 Cirru 代码（默认单行解析）
  - `-c, --cirru` - 解析多行 Cirru 代码（当有缩进时使用）
  - `-f <file>` - 从文件读取
  - `-j <json>` - 内联 JSON 字符串
  - `-s` - 从标准输入读取
  - `-J` - JSON 格式输入
  - `--json-leaf` - 直接作为叶子节点处理
  - `--refer-original <placeholder>` - 原节点占位符
  - `--refer-inner-branch <path>` - 内部分支引用路径
  - `--refer-inner-placeholder <placeholder>` - 内部分支占位符

- `cr tree delete <namespace/definition> -p <path>` - 删除指定路径的节点

- `cr tree insert-before <namespace/definition> -p <path>` - 在指定位置前插入节点

- `cr tree insert-after <namespace/definition> -p <path>` - 在指定位置后插入节点

- `cr tree insert-child <namespace/definition> -p <path>` - 插入为第一个子节点

- `cr tree append-child <namespace/definition> -p <path>` - 追加为最后一个子节点

- `cr tree swap-next <namespace/definition> -p <path>` - 与下一个兄弟节点交换

- `cr tree swap-prev <namespace/definition> -p <path>` - 与上一个兄弟节点交换

- `cr tree wrap <namespace/definition> -p <path>` - 用新结构包装节点（使用 refer-original 占位符）

**使用示例：**

```bash
# 查看节点结构
cr tree show app.main/main! -p "2,1"

# 替换单个符号（默认单行解析）
cr tree replace app.main/main! -p "0" -e "new-function"

# 替换多行代码块（使用 --cirru 标志）
cr tree replace app.main/main! -p "2" -f /tmp/code.cirru --cirru

# 删除节点
cr tree delete app.main/main! -p "1,0"

# 插入子节点
cr tree insert-child app.main/main! -p "2" -e "new-item"
```

**⚠️ 重要：精确定位的安全流程**

使用 `cr tree` 前，建议先用 `cr tree show` 确认路径：

```bash
# 1. 先查看整体结构
cr tree show app.core/my-fn -p "" -d 1

# 2. 逐层确认目标位置
cr tree show app.core/my-fn -p "2" -d 2
cr tree show app.core/my-fn -p "2,1,0"

# 3. 执行修改
cr tree replace app.core/my-fn -p "2,1,0" -e "new-value"
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

- `--cirru-one`：把输入解析为**单行 Cirru 表达式**（one-liner parser）。适合在 shell 里写一行表达式（不依赖缩进）。
- `--json-leaf`：把输入当成 **leaf 节点**。输入会直接作为 leaf 值，无需 JSON 引号包裹。
  - 传入符号：`-e 'my-symbol'`
  - 传入字符串：需要 Cirru 字符串前缀 `|` 或 `"`，例如 `-e '|my string'` 或 `-e '"my string'`

⚠️ 注意：这些开关彼此互斥（一次只用一个）。

**推荐简化规则（命令行更好写）：**

- **JSON（单行）**：优先用 `-j '<json>'` 或 `-e '<json>'`（不需要 `-J`）。
- **Cirru 单行表达式**：用 `-e '<expr>'`（`-e` 默认按 one-liner 解析；`-O/--cirru-one` 可选）。
- **Cirru 多行缩进**：用 `-f file.cirru` 或 `-s`（stdin）。
- `-J/--json-input` 主要用于 **file/stdin** 读入 JSON（如 `-f code.json -J` 或 `-s -J`）。

补充：`-e/--code` 只有在 `[...]` 内部包含 `"` 时才会自动按 JSON 解析（例如 `-e '["a"]'`）。
像 `-e '[]'` / `-e '[ ]'` 会默认按 Cirru one-liner 处理；如果你需要“空 JSON 数组”，用显式 JSON：`-j '[]'`。

如果你想在命令行里明确“这段就是 JSON”，请用 `-j '<json>'`（`-J` 是给 file/stdin 用的）。

**定义操作：**

- `cr edit def <namespace/definition> -j '<json>'` - 添加或更新定义
- `cr edit def <namespace/definition> -r -j '<json>'` - 强制覆盖已有定义
- `cr edit rm-def <namespace/definition>` - 删除定义
- `cr edit doc <namespace/definition> '<doc>'` - 更新定义的文档
- `cr edit examples <namespace/definition>` - 设置定义的示例代码（批量替换所有示例）
  - `-j '<json>'` - 内联 JSON 数组
  - `-f <file>` - 从文件读取（默认 Cirru 格式）
  - `-s` - 从 stdin 读取（默认 Cirru 格式）
  - `-J` - 使用 JSON 格式输入
  - `--clear` - 清空所有示例
- `cr edit add-example <namespace/definition>` - 添加单个示例
  - `--at <position>` - 指定插入位置（默认追加到末尾，0-based 索引）
  - `-j '<json>'` - 内联 JSON
  - `-e '<code>'` - 内联 Cirru 文本
  - `-f <file>` - 从文件读取（默认 Cirru 格式）
  - `-s` - 从 stdin 读取
  - `-O` - 使用 one-liner 解析器
  - `-J` - 使用 JSON 格式输入
- `cr edit rm-example <namespace/definition> <index>` - 删除指定索引的示例（0-based）
- `cr edit at <namespace/definition> -p <path> -o <operation> -j '<json>'` - **已弃用，请使用 `cr tree` 命令**（见下方）
  - path：逗号分隔的索引，如 "2,1,0"
  - operation："insert-before", "insert-after", "replace", "delete", "insert-child"
  - `-d <depth>` 或 `--depth <depth>`：限制结果预览深度（0=无限，默认 2）
  - 执行后会输出被修改节点的预览，方便验证修改结果

**⚠️ AST 节点精确操作已迁移**

原来的 `cr edit at` 命令已迁移到 `cr tree` 系列命令，提供更清晰的接口：

- `cr edit at ... -o replace` → `cr tree replace`
- `cr edit at ... -o delete` → `cr tree delete`
- `cr edit at ... -o insert-before` → `cr tree insert-before`
- `cr edit at ... -o insert-after` → `cr tree insert-after`
- `cr edit at ... -o insert-child` → `cr tree insert-child`

请参考上面的"精细代码树操作 (`cr tree`)"章节获取详细用法。

**命名空间操作：**

- `cr edit add-ns <namespace>` - 添加命名空间（创建最小 ns 声明）
- `cr edit add-ns <namespace> -j '<ns_json>'` - 添加带自定义 ns 代码的命名空间
- `cr edit rm-ns <namespace>` - 删除命名空间
- `cr edit imports <namespace> -j '<imports_json>'` - 更新导入规则
- `cr edit add-import <namespace> -j '<import_rule>'` - 添加单个 import 规则
- `cr edit rm-import <namespace> <source_ns>` - 移除指定来源的 import 规则
- `cr edit ns-doc <namespace> '<doc>'` - 更新命名空间文档

**模块和配置：**

- `cr edit add-module <module-path>` - 添加模块依赖
- `cr edit rm-module <module-path>` - 删除模块依赖
- `cr edit config <key> <value>` - 设置配置（key: init-fn, reload-fn, version）

**使用示例：**

```bash
# 使用内联 JSON 添加定义
cr edit def app.core/multiply -j '["defn", "multiply", ["x", "y"], ["*", "x", "y"]]'

# 使用 stdin 管道
echo '["defn", "hello", [], ["println", "|Hello"]]' | cr edit def app.core/hello -s -J

# 单行 Cirru 表达式输入（one-liner，不走 stdin/文件；-e 默认 one-liner）
cr edit def app.core/demo-one -e 'println $ str $ &+ 1 2'

# JSON leaf 输入（直接传内容作为 leaf 节点）
# 传符号：
cr edit def app.core/demo-leaf --json-leaf -e 'demo-leaf'
# 传字符串（需要 | 或 " 前缀）：
cr edit def app.core/demo-str --json-leaf -e '|demo string'

# 从文件读取（Cirru 格式）
cr edit def app.core/complex-fn -f /tmp/code.cirru

# 从文件读取（JSON 格式）
cr edit def app.core/complex-fn -f /tmp/code.json -J
```

可以使用 `--help` 参数了解更详细的用法。

---

## Calcit 语言基础

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

- `cr docs ref <keyword>` - 查询 Calcit 教程
- `cr docs api <keyword>` - 查询 API 文档
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
cr edit def app.core/multiply -j '["defn", "multiply", ["x", "y"], ["*", "x", "y"]]'
```

**更新文档和示例：**

```bash
# 更新文档
cr edit doc app.core/multiply '乘法函数，返回两个数的积'

# 设置示例（JSON 数组，每个元素是一个示例表达式）
cr edit examples app.core/multiply -j '[["multiply", "3", "4"]]'

# 从 Cirru 文件设置示例（文件中每行是一个表达式）
cr edit examples app.core/multiply -f examples.cirru

# 添加单个示例到末尾
cr edit add-example app.core/multiply -e 'multiply 5 6'

# 在指定位置插入示例（在索引 0 位置插入，成为第一个示例）
cr edit add-example app.core/multiply --at 0 -e 'multiply 1 2'

# 删除第 2 个示例（索引 1）
cr edit rm-example app.core/multiply 1

# 清空所有示例
cr edit examples app.core/multiply --clear
```

**局部修改（推荐流程）：**

```bash
# 1. 读取完整定义
cr query def app.core/add-numbers

# 2. 多次查看节点确认目标坐标
cr tree show app.core/add-numbers -p "" -d 1
cr tree show app.core/add-numbers -p "2" -d 1
cr tree show app.core/add-numbers -p "2,0"

# 3. 执行替换（使用新的 cr tree 命令）
cr tree replace app.core/add-numbers -p "2,0" -e '"*"'

# 4. 验证
cr tree show app.core/add-numbers -p "2"
```

**更新命名空间导入：**

```bash
cr edit imports app.main -j '[["app.lib", ":as", "lib"], ["app.util", ":refer", ["helper"]]]'
```
