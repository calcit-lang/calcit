---
title: "Calcit 编程 Agent 指南"
summary: "进阶 Agent/LLM 工作流：结构化编辑策略、搜索与替换模式、占位符重构、模块依赖管理"
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

详细内容已移入 [run/quick-start.md](./quick-start.md)。

**核心原则：用命令行工具（不要直接编辑文件），用 search 定位（比逐层导航快 10 倍）**

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

详细内容已移入独立文件。以下子命令各有完整文档：

- `主要运行命令` → [run/cli-options.md](./cli-options.md)
- `查询子命令` → [run/query.md](./query.md)
- `文档子命令` → [run/docs-libs.md](./docs-libs.md)
- `Cirru 语法工具` → `cr cirru show-guide`
- `精细代码树操作` → [run/edit-tree.md](./edit-tree.md)
- `代码编辑` → [run/edit-tree.md](./edit-tree.md)

### LLM 辅助：动态方法提示

在运行时调试 trait 分派时，可使用以下内置函数（低频场景，需运行期有值后调用）：

- `&methods-of value` — 列出某值的可用方法名（返回字符串列表 `[] |.foo |.bar ...`）
- `&inspect-methods value` — 打印方法与 impl 来源（调试 trait override 链，可临时插入 pipeline）
- `&impl:origin impl` — 读取 impl record 的 trait 来源
- `&trait-call Trait :method receiver & args` — 显式消歧：只调用属于指定 trait 的方法实现

> 📖 深入了解 trait 实现机制：`cr docs read traits.md` 或 `cr docs search 'trait-call'`

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

3. **填充内容**：针对占位符路径进行下一层的精细替换。

   ```bash
   cr tree replace ns/def -p '4.0.2' -j '["if", ["=", "x", "1"], "{{TRUE_BRANCH}}", "{{FALSE_BRANCH}}"]'
   ```

4. **递归迭代**：重复上述步骤直到所有占位符都被替换为最终逻辑。

**优势：**

- **精确性**：使用 JSON 格式 (`-j`) 可以完全避免 Cirru 缩进或括号解析的歧义。
- **低风险**：每次只修改一小部分，出错时容易通过 `tree show` 快速定位。
- **绕过限制**：解决某些终端对超长命令行参数的限制。

## Calcit 语言基础

详细内容已移入独立文件：

- `Cirru 语法核心概念` → [cirru-syntax.md](../cirru-syntax.md)
- `数据结构：Tuple vs Vector` → [features/tuples.md](../features/tuples.md)
- `类型标注与检查` → [features/static-analysis.md](../features/static-analysis.md)

### 其他易错点

比较容易犯的错误：

- Calcit 中字符串通过前缀区分，`|` 和 `"` 开头表示字符串。`|x` 对应 JavaScript 字符串 `"x"`。产生 JSON 时注意不要重复包裹引号。
- Calcit 采用 Cirru 缩进语法，可以理解成去掉跨行括号改用缩进的 Lisp 变种。用 `cr cirru parse` 和 `cr cirru format` 互相转化试验。
- Calcit 跟 Clojure 在语义上比较像，但语法层面只用圆括号，不用方括号花括号。

## 开发调试

详细内容已移入 [run/debugging.md](./debugging.md)。包括：

- watcher 监听模式（`cr -w` / `cr js -w` / `cr ir -w`）
- 增量触发更新（`cr edit inc`）
- 编译结果检查（`cr query error`）

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
