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
- `cr query at <namespace/definition> -p <path>` - 读取定义中指定坐标的内容
  - path：逗号分隔的索引，如 "2,1,0"，空字符串表示根节点
  - `-d <depth>` 或 `--depth <depth>`：限制 JSON 输出深度（0=无限，默认 0）
  - 输出包含：类型（leaf/list）、子节点预览、完整 JSON

**符号搜索与引用分析：**

- `cr query find <symbol> [--deps] [-f] [-n <limit>]` - 跨命名空间搜索符号
  - 默认精确匹配：返回定义位置 + 所有引用位置（带上下文预览）
  - `-f` / `--fuzzy`：模糊搜索，匹配 "namespace/definition" 格式的路径
  - `-n <limit>`：限制模糊搜索结果数量（默认 20）
  - `--deps`：包含核心命名空间（calcit.\* 开头）
- `cr query usages <namespace/definition> [--deps]` - 查找定义的所有使用位置
  - 返回：引用该定义的所有位置（带上下文预览）
  - 用于理解代码影响范围，重构前的影响分析

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

### 代码编辑 (`cr edit`)

直接编辑 compact.cirru 项目代码，支持三种输入方式：

- `--file <path>` 或 `-f <path>` - 从文件读取（默认 Cirru 格式，使用 `-J` 指定 JSON）
- `--json <string>` 或 `-j <string>` - 内联 JSON 字符串
- `--stdin` 或 `-s` - 从标准输入读取（默认 Cirru 格式，使用 `-J` 指定 JSON）

**定义操作：**

- `cr edit def <namespace/definition> -j '<json>'` - 添加或更新定义
- `cr edit def <namespace/definition> -r -j '<json>'` - 强制覆盖已有定义
- `cr edit rm-def <namespace/definition>` - 删除定义
- `cr edit doc <namespace/definition> '<doc>'` - 更新定义的文档
- `cr edit examples <namespace/definition>` - 设置定义的示例代码
  - `-j '<json>'` - 内联 JSON 数组
  - `-f <file>` - 从文件读取（默认 Cirru 格式）
  - `-s` - 从 stdin 读取（默认 Cirru 格式）
  - `-J` - 使用 JSON 格式输入
  - `--clear` - 清空所有示例
- `cr edit at <namespace/definition> -p <path> -o <operation> -j '<json>'` - 在指定路径操作
  - path：逗号分隔的索引，如 "2,1,0"
  - operation："insert-before", "insert-after", "replace", "delete", "insert-child"
  - `-d <depth>` 或 `--depth <depth>`：限制结果预览深度（0=无限，默认 2）
  - 执行后会输出被修改节点的预览，方便验证修改结果

**⚠️ 重要：精确编辑的安全流程**

使用 `edit at` 进行局部修改前，**必须先多次使用 `query at` 确认坐标**，避免错误覆盖代码：

```bash
# 步骤1: 先读取整体结构，了解根节点 (用 -d 1 限制深度减少输出)
cr query at app.core/my-fn -p "" -d 1

# 步骤2: 逐层深入，确认目标位置
cr query at app.core/my-fn -p "2" -d 1      # 查看第3个子节点
cr query at app.core/my-fn -p "2,1" -d 1    # 继续深入
cr query at app.core/my-fn -p "2,1,0"       # 确认最终目标

# 步骤3: 确认无误后再执行修改
cr edit at app.core/my-fn -p "2,1,0" -o replace -j '"new-value"'

# 步骤4: 验证修改结果
cr query at app.core/my-fn -p "2,1"
```

**命名空间操作：**

- `cr edit add-ns <namespace>` - 添加命名空间（创建最小 ns 声明）
- `cr edit add-ns <namespace> -j '<ns_json>'` - 添加带自定义 ns 代码的命名空间
- `cr edit rm-ns <namespace>` - 删除命名空间
- `cr edit imports <namespace> -j '<imports_json>'` - 更新导入规则
- `cr edit require <namespace> -j '<require_rule>'` - 添加单个 require 规则
- `cr edit rm-require <namespace> <source_ns>` - 移除指定来源的 require 规则
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

# 清空示例
cr edit examples app.core/multiply --clear
```

**局部修改（推荐流程）：**

```bash
# 1. 读取完整定义
cr query def app.core/add-numbers

# 2. 多次 query at 确认目标坐标
cr query at app.core/add-numbers -p "" -d 1
cr query at app.core/add-numbers -p "2" -d 1
cr query at app.core/add-numbers -p "2,0"

# 3. 执行替换
cr edit at app.core/add-numbers -p "2,0" -o replace -j '"*"'

# 4. 验证
cr query at app.core/add-numbers -p "2"
```

**更新命名空间导入：**

```bash
cr edit imports app.main -j '[["app.lib", ":as", "lib"], ["app.util", ":refer", ["helper"]]]'
```
