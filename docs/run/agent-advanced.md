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
  - "cr exec"
  - "calcit.cli/search-def"
  - "calcit.cli/tree-replace"
  - "calcit.cli/edit-def"
---

# Calcit 编程 Agent 指南

本文档为 AI Agent 提供 Calcit 项目的操作指南。

本文定位为 Agents 约束与完整操作手册：覆盖硬前置步骤、命令边界、复杂重构与系统化排障。`docs/CalcitAgent.md` 用于查询与局部编辑速查，不替代本文中的约束规则。

## 🚀 快速开始（新 LLM 必读）

详细内容已移入 [run/quick-start.md](./quick-start.md)。

**核心原则：用 `cr exec` + `calcit.cli/*`（不要直接编辑文件），用 `search-def` 定位（比逐层导航快 10 倍）**

---

## 🔧 零 Shell 转义方案：`cr exec` + `calcit.cli/*`

传统 `cr` 命令通过 Shell 参数传代码，`$` `` ` `` `|` `>` `<` `&` `;` `(` `)` `!` `?` `*` `[` `]` 等字符需要转义。
**`cr exec` 固定从 stdin 读取，完全绕过 Shell 转义**，配合 `calcit.cli/*` 内建函数，所有操作都变成纯 Cirru 函数调用。

```cirru.cli
; 查询命名空间
calcit.cli/list-ns $ {}
; 列定义
calcit.cli/list-defs $ {} (:namespace |app.core)
; 查看定义
calcit.cli/show-def $ {} (:target |app.core/main!)
; 预览前 N 行
calcit.cli/peek-def $ {} (:target |app.core/main!) (:lines 5)
; 搜索关键字路径和叶子预览
calcit.cli/search-def $ {} (:target |app.core/main!) (:keyword |keyword)
; 跨 ns 找符号
calcit.cli/find-symbol $ {} (:symbol |main!)
; 查看类型标注
calcit.cli/show-schema $ {} (:target |app.core/main!)
; 查看示例
calcit.cli/list-examples $ {} (:target |app.core/main!)
; 查看配置
calcit.cli/list-config $ {}
; 查看模块依赖
calcit.cli/list-modules $ {}
; 查看 AST 子树
calcit.cli/tree-show $ {} (:target |app.core/main!) (:path |3.1.0)
```

```bash
# heredoc 传入，内容完全不受 Shell 转义
cr project.cirru exec << 'END'
calcit.cli/list-ns $ {}
calcit.cli/list-defs $ {} (:namespace |app.core)
calcit.cli/peek-def $ {} (:target |app.core/main!) (:lines 8)
END

# 或管道
echo 'calcit.cli/list-config $ {}' | cr project.cirru exec
```

> 💡 `cr exec` 固定从 stdin 读代码；短表达式仍可用 `cr eval 'code'`。`calcit.cli/*` 为实验性内建函数，通过 `register_import_proc` 注册，可直接在 Cirru 代码中调用；统一形式为 `calcit.cli/f $ {} (:key value) ...`。完整函数列表见文末参考表。

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

- **`calcit.cirru` / `compact.cirru`** - 这是 Calcit 程序的运行时快照格式；推荐使用 `calcit.cirru`，旧文件名 `compact.cirru` 仍兼容，必须使用 `calcit.cli/*` 写函数或传统 `cr edit`/`cr tree` 进行修改

这两个文件的格式对空格和结构极其敏感，直接文本修改会破坏文件结构。请使用 `cr exec` + `calcit.cli/*` 进行代码查询和修改。

## Calcit 与 Cirru 的关系

- **Calcit** 是编程语言本身（一门类似 Clojure 的函数式编程语言）
- **Cirru** 是语法格式（缩进风格的 S-expression，类似去掉括号改用缩进的 Lisp）
- **关系**：Calcit 代码使用 Cirru 语法书写和存储

**具体体现：**

- `calcit.cirru`（兼容旧文件名 `compact.cirru`）使用 Cirru 语法存储，优先用 `calcit.cli/*` 写函数修改
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

> 本文档工作流以 `calcit.cli/*` 为主。尚未提供等价函数的 CLI 子命令，见 [RFC](../RFCs/06-29-cr-exec-cli-builtins-rfc.md)「未覆盖的 CLI 功能」。

### LLM 辅助：动态方法提示

在运行时调试 trait 分派时，可使用以下内置函数（低频场景，需运行期有值后调用）：

- `&methods-of value` — 列出某值的可用方法名（返回字符串列表 `[] |.foo |.bar ...`）
- `&inspect-methods value` — 打印方法与 impl 来源（调试 trait override 链，可临时插入 pipeline）
- `&impl:origin impl` — 读取 impl record 的 trait 来源
- `&trait-call Trait :method receiver & args` — 显式消歧：只调用属于指定 trait 的方法实现

> 📖 深入了解 trait 实现机制：`cr docs read traits.md` 或 `cr docs search 'trait-call'`

### 复杂表达式分段组装策略 (Incremental Assembly) ⭐⭐⭐

当需要构造非常复杂的嵌套结构（例如递归循环、多级 `let` 或 `if`）时，直接通过 Shell 传 Cirru 代码容易遇到转义、括号对齐或长度限制等问题。推荐使用**分段占位组装**策略：

简单提示：

- 占位符统一使用 `{{NAME}}` 风格，例如 `{{BODY}}`、`{{TRUE_BRANCH}}`；
- 大表达式可以先用 `calcit.cli/show-def $ {} (:target |ns/def)` 看整体；
- 再用 `calcit.cli/tree-show $ {} (:target |ns/def) (:path |3.1.0)` 深入某个片段；
- 真正填充时，优先用 `calcit.cli/search-replace` 找占位符，不唯一时再退回 `calcit.cli/tree-replace`。

1. **确立骨架**：先替换目标节点为一个带有占位符的简单结构。

   ```cirru.cli
calcit.cli/tree-replace $ {} (:target |ns/def) (:path |4.0) (:code $ quote $ let ((x 1)) {{BODY}})
   ```

2. **定位占位符**：使用 `tree-show` 确认占位符的具体路径。

   ```cirru.cli
calcit.cli/tree-show $ {} (:target |ns/def) (:path |4.0)
   ```

3. **填充内容**：针对占位符路径进行下一层的精细替换。

   ```cirru.cli
calcit.cli/tree-replace $ {} (:target |ns/def) (:path |4.0.2) (:code $ quote $ if (= x 1) {{TRUE_BRANCH}} {{FALSE_BRANCH}})
   ```

4. **递归迭代**：重复上述步骤直到所有占位符都被替换为最终逻辑。

**优势：**

- **精确性**：`:code` 等 AST 参数用 `(:code $ quote |leaf)` / `(:code $ quote $ expr ...)`（`:cirru-quote`），preprocess 会校验类型。
- **低风险**：每次只修改一小部分，出错时容易通过 `tree-show` 快速定位。
- **绕过限制**：解决某些终端对超长命令行参数的限制。

```bash
cr project.cirru exec << 'END'
calcit.cli/tree-replace $ {} (:target |ns/def) (:path |4.0) (:code $ quote $ let ((x 1)) {{BODY}})
calcit.cli/tree-show $ {} (:target |ns/def) (:path |4.0)
calcit.cli/tree-replace $ {} (:target |ns/def) (:path |4.0.2) (:code $ quote $ if (= x 1) {{TRUE_BRANCH}} {{FALSE_BRANCH}})
END
```

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
- 增量触发更新（`calcit.cli/trigger-inc $ {} (:changed …)`，等价 `cr edit inc`）
- 编译结果检查（`calcit.cli/show-error $ {}` 或 `cr query error`）

## 文档支持

遇到疑问时使用：

- `calcit.cli/docs-search $ {} (:keyword |keyword)` — 在 `docs/` 与 `~/.config/calcit/docs` 中 grep markdown（简化版 `cr docs search`）
- `calcit.cli/cirru-show-guide $ {}` — 读取 Cirru 语法指南
- `calcit.cli/read-text-file $ {} (:path |path)` — 读取任意文档文件
- `cr docs agents [<heading> ...] [--full]` — 结构化 Agent 指南（**暂无 calcit.cli 等价**）
- `cr docs read` / `sections` / `remote-libs` 等 — 见 [debugging.md](./debugging.md)

查询项目代码（`calcit.cli/*` 函数）：

| 需求               | `calcit.cli/*` 调用                                                                 |
| ------------------ | ----------------------------------------------------------------------------------- |
| 列出命名空间       | `calcit.cli/list-ns $ {} `                              |
| 列出某 ns 的定义   | `calcit.cli/list-defs $ {} (:namespace \|app.core)`    |
| 预览定义前 N 行    | `calcit.cli/peek-def $ {} (:target \|app.core/main!) (:lines 5)` |
| 查看完整定义       | `calcit.cli/show-def $ {} (:target \|app.core/main!)`   |
| 搜索关键字路径     | `calcit.cli/search-def $ {} (:target \|ns/def) (:keyword \|keyword)` |
| 跨 ns 找符号       | `calcit.cli/find-symbol $ {} (:symbol \|symbol)`       |
| 查看类型标注       | `calcit.cli/show-schema $ {} (:target \|ns/def)`        |
| 查看示例           | `calcit.cli/list-examples $ {} (:target \|ns/def)`     |
| 查找引用           | `calcit.cli/list-usages $ {} (:target \|ns/def)`        |
| 查看配置           | `calcit.cli/list-config $ {} `                          |
| 查看 AST 子树      | `calcit.cli/tree-show $ {} (:target \|ns/def) (:path \|3.1.0)` |
| 运行时错误堆栈     | `calcit.cli/show-error $ {}`                                                        |
| 触发热更新增量     | `calcit.cli/trigger-inc $ {} (:changed \|app.core/my-fn)` |
| 文档关键字搜索     | `calcit.cli/docs-search $ {} (:keyword \|keyword)`                                   |
| 调用图分析         | `calcit.cli/analyze-call-graph $ {} `                    |

---

## 代码修改示例

### 添加新函数

```cirru.cli
calcit.cli/edit-def $ {} (:target |app.core/multiply) (:code $ quote $ defn multiply (x y) (* x y))
```

### 基本操作

```cirru.cli
; 添加新函数
calcit.cli/edit-def $ {} (:target |app.core/multiply) (:code $ quote $ defn multiply (x y) (* x y))

; 覆盖已有定义（`(:overwrite true)`）
calcit.cli/edit-def $ {} (:target |app.core/multiply) (:code $ quote $ defn multiply (x y) (* x y)) (:overwrite true)

; 添加 :refer import
calcit.cli/add-import $ {} (:namespace |app.main) (:source-ns |app.util) (:refer-sym |helper)

; 触发热更新（watcher 模式下写入 .compact-inc.cirru）
calcit.cli/trigger-inc $ {} (:changed |app.core/my-fn)
```

### 修改定义工作流

```cirru.cli
; 1. 搜索定位（返回 path + leaf-preview）
calcit.cli/search-def $ {} (:target |ns/def) (:keyword |pattern)

; 2. 查看节点上下文
calcit.cli/tree-show $ {} (:target |ns/def) (:path |3.1.0)

; 3. 执行替换
calcit.cli/tree-replace $ {} (:target |ns/def) (:path |3.1.0) (:code $ quote |new-value)

; 4. 或搜索替换叶子
calcit.cli/search-replace $ {} (:target |ns/def) (:pattern |old-sym) (:replacement |new-sym)

; 5. 验证写回结果
calcit.cli/show-def $ {} (:target |ns/def)
```

```bash
cr project.cirru exec << 'END'
calcit.cli/search-def $ {} (:target |ns/def) (:keyword |pattern)
calcit.cli/tree-show $ {} (:target |ns/def) (:path |3.1.0)
calcit.cli/tree-replace $ {} (:target |ns/def) (:path |3.1.0) (:code $ quote |new-value)
calcit.cli/show-def $ {} (:target |ns/def)
END
```

---

## 🔧 实战重构场景

以下是开发中最常见的局部修复和重构操作。已提供 `calcit.cli/*` 的优先使用；未覆盖的标注为「暂无等价」。

### 提取子表达式为新定义（`split-def`）

**场景：** 函数体内某个嵌套子表达式太复杂，想拆成独立的命名定义。

```cirru.cli
; 1. 搜索并定位目标子表达式
calcit.cli/search-def $ {} (:target |app.core/process-data) (:keyword |complex-call)

; 2. 查看上下文确认路径
calcit.cli/tree-show $ {} (:target |app.core/process-data) (:path |3.2.1)

; 3. 提取为新定义
calcit.cli/split-def $ {} (:target |app.core/process-data) (:path |3.2.1) (:new-name |extracted-calc)

; 4. 验证结果
calcit.cli/show-def $ {} (:target |app.core/extracted-calc)
calcit.cli/show-def $ {} (:target |app.core/process-data)
```

### 重命名定义（`rename-def`）

**场景：** 定义名字需要在同一命名空间内改名。

```cirru.cli
; 1. 确认有哪些地方引用到
calcit.cli/list-usages $ {} (:target |app.core/old-name)

; 2. 搜索所有引用位置
calcit.cli/search-def $ {} (:target |app.core/caller-fn) (:keyword |old-name)

; 3. 重命名定义
calcit.cli/rename-def $ {} (:target |app.core/old-name) (:new-name |new-name)

; 4. 批量更新引用
calcit.cli/search-replace $ {} (:target |app.core/caller-fn) (:pattern |old-name) (:replacement |new-name)
```

### 迁移定义到另一命名空间（`mv-def`）

**场景：** 某函数放错了命名空间，需要迁移。

```cirru.cli
calcit.cli/mv-def $ {} (:source |app.util/helper-fn) (:target |app.core/helper-fn)
calcit.cli/add-import $ {} (:namespace |app.main) (:source-ns |app.core) (:refer-sym |helper-fn)
```

```cirru.cli
; watcher 模式下触发热更新
calcit.cli/trigger-inc $ {} (:changed |app.core/helper-fn)
```

### 在定义内移动 / 复制 AST 节点（`tree-cp` / `tree-mv`）

**场景：** 函数体内某个子表达式需要移到另一位置，或复制用于多处。

```cirru.cli
; 定位节点
calcit.cli/search-def $ {} (:target |app.core/main-fn) (:keyword |process)
calcit.cli/tree-show $ {} (:target |app.core/main-fn) (:path |3.1.2)

; 复制或移动（position: |before |after |prepend-child |append-child |replace）
calcit.cli/tree-cp $ {} (:target |app.core/main-fn) (:from-path |3.1.2) (:to-path |3.0) (:position |after)
calcit.cli/tree-mv $ {} (:target |app.core/main-fn) (:from-path |3.1.2) (:to-path |3.0) (:position |after)
```

### 包裹 / 拆包 / 提升节点（`tree-wrap` / `tree-unwrap` / `tree-raise`）

**场景：** 临时包裹一层 `println` 调试、反向拆掉包装层、或用子节点替换掉父节点。

```cirru.cli
; wrap：模板中用 self 引用原节点
calcit.cli/tree-wrap $ {} (:target |app.core/main-fn) (:path |3.1.2) (:wrapper-code $ quote $ println self)

; unwrap / raise
calcit.cli/tree-unwrap $ {} (:target |app.core/main-fn) (:path |3.1.2)
calcit.cli/tree-raise $ {} (:target |app.core/main-fn) (:path |3.1.2)
```

### 批量重命名局部变量

**场景：** 某函数内某个局部变量名需要统一改掉。

```cirru.cli
; 搜索替换所有匹配的 leaf
calcit.cli/search-replace $ {} (:target |app.core/process) (:pattern |old-var) (:replacement |new-var)
```

---

## ⚠️ 常见陷阱和最佳实践

### 1. 路径索引动态变化问题 ⭐⭐⭐

**核心原则：** 删除/插入会改变同级后续节点索引。

**批量修改策略：**

- **从后往前操作**（推荐）：先删大索引，再删小索引
- **单次操作后重新搜索**：每次修改立即用 `calcit.cli/search-def` 更新路径
- **整体重写**：优先用 `calcit.cli/edit-def` 带 `(:overwrite true)` 覆盖；根路径 `tree-replace` 只保留给明确需要根节点级别改写的场景

非法 path 会抛出明确错误，例如 `tree-show: invalid path 'bad.path': segment 'bad' is not an unsigned integer`。

### 1.5 根路径整体替换的边界 ⭐⭐⭐

`calcit.cli/tree-replace` 的 path 不能为空（写操作不允许 root path）。当你需要完整替换一个定义体时：

- 更推荐 `calcit.cli/edit-def $ {} (:target |ns/def) (:code $ quote $ defn ...) (:overwrite true)`
- 先在 snippet 里组织完整定义，再一次性覆盖，验证也更直接
- 替换成功后仍应立刻执行 `calcit.cli/show-def` 确认写回结构符合预期

### 2. 输入格式：Cirru one-liner 字符串 ⭐⭐⭐

`calcit.cli/*` 写函数的 `code` 参数必须是 **Cirru one-liner 字符串**，用 `"|..."` 双引号包围以包含空格：

| 场景           | 写法示例                                              | 说明                     |
| -------------- | ----------------------------------------------------- | ------------------------ |
| AST path       | `calcit.cli/tree-show $ {} (:path \|3.1.0)`                    | path 必须是字符串        |
| 表达式         | `"\|defn add (a b) (+ a b)"`                          | 完整 Cirru one-liner     |
| 原子符号 leaf  | `calcit.cli/tree-replace $ {} (:path \|3.1.0) (:code $ quote \|new-symbol))`      | 替换为 leaf              |
| 字符串 leaf    | `calcit.cli/search-replace $ {} (:pattern \|old) (:replacement \|new text)`      | 搜索替换 leaf            |
| 覆盖已有定义   | `calcit.cli/edit-def $ {} (:code $ quote $ code) (:overwrite true)`                | 加 `(:overwrite true)`        |

**实战示例：**

```cirru.cli
; ✅ 替换表达式
calcit.cli/tree-replace $ {} (:target |app.main/fn) (:path |2) (:code $ quote $ println |hello)

; ✅ 替换 leaf
calcit.cli/tree-replace $ {} (:target |app.main/fn) (:path |2.0) (:code $ quote |new-symbol)

; ✅ 搜索替换 leaf
calcit.cli/search-replace $ {} (:target |app.main/fn) (:pattern |old-var) (:replacement |new-var)
```

### 3. Cirru 字符串和数据类型 ⭐⭐

**Cirru 字符串前缀：**

| Cirru 写法     | JSON 等价      | 使用场景     |
| -------------- | -------------- | ------------ |
| `\|hello`      | `"hello"`      | 推荐，简洁   |
| `"hello"`      | `"hello"`      | 也可以       |
| `\|a b c`      | `"a b c"`      | 包含空格     |
| `\|[tag] text` | `"[tag] text"` | 包含特殊字符 |

**不放心修改是否正确？** 每步后用 `calcit.cli/tree-show` 验证。

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

`calcit.cli/edit-def` 和 `calcit.cli/tree-replace` 的 code 参数通过 Cirru one-liner 传入，建议单次不超过 **1000 字符**。

**大资源处理建议：**
如果需要修改复杂的长函数，不要尝试一次性替换整个定义。应先构建主体结构，使用占位符，统一写成 `{{PLACEHOLDER_FEATURE}}` 这种花括号形式，并注意避免重复，然后通过 `calcit.cli/search-replace` 或 `calcit.cli/tree-replace` 做精准的分段替换。

`calcit.cli/show-def` / `calcit.cli/tree-show` 面向 LLM 脚本调用，返回稳定字符串；`tree-show` 的 `(:max-lines N)` 控制最大输出行数（默认 80）。

### 5. 命名空间 import 操作 ⭐⭐⭐

`calcit.cli/add-import` **仅支持 `:refer` 单符号导入**：

```cirru.cli
; ✅ 正确：添加 :refer import
calcit.cli/add-import $ {} (:namespace |app.main) (:source-ns |app.util) (:refer-sym |helper)

; ✅ 分两次添加 :as 和 :refer（Calcit 不支持合并写法）
calcit.cli/add-import $ {} (:namespace |app.main) (:source-ns |app.schema) (:refer-sym |schema)
calcit.cli/add-import $ {} (:namespace |app.main) (:source-ns |app.schema) (:refer-sym |Op)
```

**常见陷阱：**

❌ **在 Cirru 源码中合并 `:as` 和 `:refer` 到同一条 import 规则**

```cirru.no-check
;; ❌ 错误：:refer 部分被静默丢弃，Op 无法被找到
ns app.main $ :require
  app.schema :as schema :refer $ Op

;; ✅ 正确：拆成两条独立规则
ns app.main $ :require
  app.schema :as schema
  app.schema :refer $ Op
```

> `add-import` 不支持 `:as` / `:rename` / 多条规则批量写入。复杂 import 操作需传统 `cr edit imports` / `cr edit add-ns`。

### 6. 推荐工作流程

**基本流程（search 快速定位 ⭐⭐⭐）：**

```cirru.cli
; 1. 快速定位
calcit.cli/search-def $ {} (:target |ns/def) (:keyword |target)

; 2. 查看上下文
calcit.cli/tree-show $ {} (:target |ns/def) (:path |3.1.0)

; 3. 执行修改
calcit.cli/tree-replace $ {} (:target |ns/def) (:path |3.1.0) (:code $ quote |new-value)

; 4. 验证
calcit.cli/show-def $ {} (:target |ns/def)
```

**新手提示：**

- 不知道目标在哪？用 `search-def` 快速找到所有匹配
- 想了解代码结构？用 `tree-show` 逐层探索
- 需要批量重命名 leaf？用 `search-replace`
- 不确定修改是否正确？每步后用 `tree-show` 验证

### 7. 特殊字符：完全绕过 Shell ⭐⭐⭐

Calcit 函数名中的 `?`, `->`, `!` 等字符在 bash/zsh 中有特殊含义。使用 `cr exec` + `calcit.cli/*` 时**完全不需要转义**：

```cirru.cli
; 含 $ ` | > < & ; 等特殊字符，全部可以自由书写
calcit.cli/search-def $ {} (:target |ns/main!) (:keyword |$data)
calcit.cli/peek-def $ {} (:target |app.core/valid?) (:lines 10)
calcit.cli/show-def $ {} (:target |app.main/valid?)
```

```bash
cr project.cirru exec << 'END'
calcit.cli/list-defs $ {} (:namespace |app.core)
calcit.cli/search-def $ {} (:target |app.core/process!) (:keyword |item)
END
```

### 8. 多命令链式调用 ⭐⭐⭐

**推荐：用 `cr exec` + heredoc 一次传入多条 `calcit.cli/*` 调用**，无转义风险、无路径索引漂移误判：

```bash
cr project.cirru exec << 'END'
calcit.cli/peek-def $ {} (:target |app.core/main!) (:lines 5)
calcit.cli/search-def $ {} (:target |app.core/main!) (:keyword |keyword)
calcit.cli/tree-show $ {} (:target |app.core/main!) (:path |3.1.0)
calcit.cli/tree-replace $ {} (:target |app.core/main!) (:path |3.1.0) (:code $ quote |updated)
calcit.cli/show-def $ {} (:target |app.core/main!)
END
```

---

## 🔄 完整功能开发示例

以下展示从零开始添加新函数的完整流程，是最常见的日常开发场景。

### 步骤 1：确认目标命名空间和现有代码

```cirru.cli
calcit.cli/list-ns $ {}
calcit.cli/list-defs $ {} (:namespace |app.util)
calcit.cli/peek-def $ {} (:target |app.util/format-date) (:lines 5)
calcit.cli/show-def $ {} (:target |app.util/format-date)
```

### 步骤 2：用 exec 快速验证写法

在真正写入项目前，先用 `cr exec` 验证逻辑思路：

```bash
cr project.cirru exec << 'END'
string->number |123
END
```

```bash
cr project.cirru exec << 'END'
let ((x 10) (y 20)) (+ x y)
END
```

> 💡 有类型警告时 exec 会以错误退出——正好可以提前发现用法错误。

### 步骤 3：添加新定义

```cirru.cli
calcit.cli/edit-def $ {} (:target |app.util/calculate-discount) (:code $ quote $ defn calculate-discount (price rate) (* price (- 1 rate)))
calcit.cli/show-def $ {} (:target |app.util/calculate-discount)
```

### 步骤 4：在调用方添加 import 并使用

```cirru.cli
calcit.cli/list-defs $ {} (:namespace |app.core)
calcit.cli/add-import $ {} (:namespace |app.core) (:source-ns |app.util) (:refer-sym |calculate-discount)
calcit.cli/search-def $ {} (:target |app.core/checkout) (:keyword |total-price)
calcit.cli/tree-replace $ {} (:target |app.core/checkout) (:path |3.2.1) (:code $ quote $ calculate-discount total-price 0.1)
```

### 步骤 5：触发热更新并验证

```cirru.cli
calcit.cli/trigger-inc $ {} (:changed |app.util/calculate-discount,app.core/checkout)
calcit.cli/show-def $ {} (:target |app.util/calculate-discount)
calcit.cli/show-def $ {} (:target |app.core/checkout)
```

> `cr --check-only` 暂无 calcit.cli 等价；可重新运行 `cr js` / `cr` 验证。

### 常见失误快速修复

```cirru.cli
; 忘记 import → unknown symbol
calcit.cli/add-import $ {} (:namespace |app.core) (:source-ns |app.util) (:refer-sym |calculate-discount)

; 函数参数顺序传错 → 定位并修改调用
calcit.cli/search-def $ {} (:target |app.core/checkout) (:keyword |calculate-discount)
calcit.cli/tree-replace $ {} (:target |app.core/checkout) (:path |3.2.1) (:code $ quote |calculate-discount)
```

> `edit rename` 拼写错误可用 `calcit.cli/rename-def` 修正。

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

## 📋 `calcit.cli/*` 函数参考

通过 `cr exec` + `calcit.cli/*` 函数，查询与写操作都变成纯 Cirru 函数调用，绕过 Shell 转义。

| 函数             | map 选项键                                   | 说明                       |
| ---------------- | -------------------------------------------- | -------------------------- |
| `list-ns`        | `[file-path]` | 列出命名空间               |
| `list-defs`      | `[file-path]` `namespace`                      | 列出定义                   |
| `show-def`       | `[file-path]` `target`                         | 返回完整代码               |
| `peek-def`       | `[file-path]` `target` `[lines]`               | 预览前 N 行                |
| `search-def`     | `[file-path]` `target` `keyword`               | 搜索关键字路径和叶子预览   |
| `find-symbol`    | `[file-path]` `symbol`                         | 跨 ns 找符号               |
| `show-schema`    | `[file-path]` `target`                         | 查看类型标注               |
| `list-examples`  | `[file-path]` `target`                         | 列出示例                   |
| `list-usages`    | `[file-path]` `target`                         | 查找引用位置               |
| `list-config`    | `[file-path]` | 查看配置                   |
| `list-modules`   | `[file-path]` | 查看模块依赖               |
| `tree-show`      | `[file-path]` `target` `[path]` `[max-lines]`  | 查看 AST 子树              |
| `tree-replace`   | `[file-path]` `target` `path` `code` (`:cirru-quote`)           | 替换 AST 节点              |
| `search-replace` | `[file-path]` `target` `pattern` `replacement` | 搜索替换叶子节点           |
| `edit-def`       | `[file-path]` `target` `code` (`:cirru-quote`) `[overwrite]`    | 创建/更新定义              |
| `add-import`     | `[file-path]` `namespace` `source-ns` `refer-sym` | 添加 `:refer` import    |
| `tree-delete`    | `[file-path]` `target` `path`                  | 删除 AST 节点              |
| `tree-insert`    | `[file-path]` `target` `path` `code` (`:cirru-quote`) `position`| 插入节点                   |
| `tree-wrap`      | `[file-path]` `target` `path` `wrapper-code` (`:cirru-quote`)   | 包裹节点（`self` 占位）    |
| `tree-unwrap`    | `[file-path]` `target` `path`                  | 拆包 list 节点             |
| `tree-raise`     | `[file-path]` `target` `path`                  | 子节点替换父节点           |
| `tree-cp`        | `[file-path]` `target` `from-path` `to-path` `position` | 复制 AST 子树     |
| `tree-mv`        | `[file-path]` `target` `from-path` `to-path` `position` | 移动 AST 子树     |
| `rename-def`     | `[file-path]` `target` `new-name`              | 重命名定义                 |
| `rm-def`         | `[file-path]` `target`                         | 删除定义                   |
| `mv-def`         | `[file-path]` `source` `target`                | 跨 ns 迁移定义             |
| `split-def`      | `[file-path]` `target` `path` `new-name`       | 提取子表达式为新定义       |
| `add-ns`         | `[file-path]` `namespace` `[code]`             | 创建命名空间               |
| `rm-import`      | `[file-path]` `namespace` `source-ns`          | 移除 import 规则           |
| `edit-doc`       | `[file-path]` `target` `doc`                   | 更新定义文档               |
| `edit-schema`    | `[file-path]` `target` `schema-code`           | 更新类型标注               |
| `add-example`    | `[file-path]` `target` `code` `[index]`        | 添加示例                   |
| `show-error`     | `[error-file]`                               | 读取 `.calcit-error.cirru` |
| `set-imports`    | `[file-path]` `namespace` `rules-code`         | 全量替换 import 规则       |
| `format-file`    | `[file-path]` | 格式化 snapshot 文件       |
| `show-doc`       | `[file-path]` `target`                         | 读取定义文档               |
| `list-tags`      | `[file-path]` `target`                         | 读取 tags                  |
| `set-tags`       | `[file-path]` `target` `tags`                  | 设置 tags                  |
| `rm-example`     | `[file-path]` `target` `index`                 | 删除示例                   |
| `clear-examples` | `[file-path]` `target`                         | 清空示例                   |
| `set-examples`   | `[file-path]` `target` `examples-code`         | 重置示例列表               |
| `tree-replace-leaf` | `[file-path]` `target` `pattern` `replacement-code` | 批量替换匹配 leaf   |
| `tree-swap-next` | `[file-path]` `target` `path`                  | 与下一兄弟交换             |
| `tree-swap-prev` | `[file-path]` `target` `path`                  | 与上一兄弟交换             |
| `tree-batch-delete` | `[file-path]` `target` `paths` (`(:: :list :string)`) | 批量删除 path 列表 |
| `tree-rewrite`   | `[file-path]` `target` `path` `template-code` `refs` | 带 ref 模板替换      |
| `set-config`     | `[file-path]` `key` `value` `[entry]`          | 设置 init-fn/reload-fn/version |
| `add-module`     | `[file-path]` `module-path` `[entry]`        | 添加模块依赖               |
| `rm-module`      | `[file-path]` `module-path` `[entry]`        | 移除模块依赖               |
| `cirru-parse`    | `code` `[one-liner]`                         | Cirru → JSON 字符串        |
| `cirru-format`   | `json`                                       | JSON → Cirru 文本          |
| `read-text-file` | `path`                                       | 读取任意文本文件           |

```bash
cr project.cirru exec << 'END'
calcit.cli/list-ns $ {}
calcit.cli/list-defs $ {} (:namespace |app.core)
calcit.cli/peek-def $ {} (:target |app.core/main!) (:lines 8)
calcit.cli/search-def $ {} (:target |app.core/main!) (:keyword |state)
calcit.cli/tree-show $ {} (:target |app.core/main!) (:path |3.1.0)
END
```

---

## check-md：验证 `calcit.cli` 选项类型

Rust 侧 `calcit_cli_specs.rs` 是选项类型的唯一来源；preprocess 会据此检查 map 选项（未知 key、缺必填、类型错误）。文档里的 **`cirru.cli` 代码块**会走 `check-md` 的 preprocess 路径（不执行 runtime；省略的 `:file-path` 在 `cr <file> exec` / check-md 中会 fallback 到 entry 文件）。

### 运行命令

```bash
# 仓库内验证本文档全部 cirru 块（含 calcit.cli 类型检查）
cr calcit/test.cirru docs check-md docs/run/agent-advanced.md -d calcit/test.cirru

# 等价写法（显式 entry）
cr docs check-md docs/run/agent-advanced.md -d calcit/test.cirru
```

块类型速查：`cirru.cli` = 解析 + preprocess（适合 `calcit.cli/*`）；`cirru` = 完整 eval；`cirru.no-check` = 仅语法示意，不参与类型检查。

### 应通过的 `cirru.cli` 案例（纳入 check-md）

下列块使用仓库内真实路径 `calcit/test.cirru`，专门覆盖选项 map 的常见写法：

```cirru.cli
; 空 map 即可（:file-path 默认取 cr entry）
calcit.cli/list-ns $ {}
```

```cirru.cli
; 必填 + 可选数字（:lines 有默认值，可省略）
calcit.cli/peek-def $ {} (:target |app.main/main!) (:lines 5)
```

```cirru.cli
; 多必填 string
calcit.cli/search-def $ {} (:target |app.main/main!) (:keyword |main)
```

```cirru.cli
; 无必填项的空 map
calcit.cli/cirru-show-guide $ {}
```

```cirru.cli
; bool 选项（:overwrite 默认 false，显式传入）
calcit.cli/edit-def $ {} (:target |app.main/reload!) (:code $ quote $ defn reload! () nil) (:overwrite true)
```

```cirru.cli
; 多可选 string（trigger-inc 其余键可省略）
calcit.cli/trigger-inc $ {} (:changed |app.main/main!)
```

### 预期失败案例（手动验证，勿放入 `cirru.cli` 块）

`check-md` 要求文档内每个 cirru 块都通过；**故意写错的选项应放在 bash 里用 `cr eval` 验证**，避免拖垮整份文档的 check-md。

```bash
# 拼写错误 → W_CLI_OPTION_UNKNOWN_KEY
cr calcit/test.cirru eval 'calcit.cli/list-ns $ {} (:file-pth |x)'

# 缺少必填 → W_CLI_OPTION_MISSING_REQUIRED（peek-def 缺 :target）
cr calcit/test.cirru eval 'calcit.cli/peek-def $ {}'

# 类型错误 → W_CLI_OPTION_TYPE_MISMATCH
cr calcit/test.cirru eval 'calcit.cli/peek-def $ {} (:target |app.main/main!) (:lines |bad)'

# 未知 key + 缺必填（可同时报多条告警）
cr calcit/test.cirru eval 'calcit.cli/peek-def $ {} (:file-pth |x)'
```

### 选项类型告警码

| 告警码 | 典型原因 | 修复 |
| ------ | -------- | ---- |
| `W_CLI_OPTION_UNKNOWN_KEY` | key 拼写错误（如 `:file-pth`） | 对照函数文档或 `cr query peek calcit.cli/list-ns` 的 Options |
| `W_CLI_OPTION_MISSING_REQUIRED` | 未传必填项（如 `peek-def` 缺 `:target`） | 补全 map 中的必填 tag |
| `W_CLI_OPTION_TYPE_MISMATCH` | 值类型不符（如 `:lines \|bad`） | `:string` 用 `\|text` 或 tag；`:number` 用数字；`:bool` 用 `true`/`false` |

统一调用形式：`calcit.cli/<fn> $ {} (:key value) ...`；key 必须是 tag（`:target` 等）。`:file-path` 可省略，默认取 `cr` 启动时的 entry 文件。

---

## 常见错误排查

### 快速诊断流程

当 watcher 提示有错误或行为异常时，按以下顺序排查：

1. 查看最新错误堆栈：`calcit.cli/show-error $ {}` 或 `cr query error`
2. 用 `cr --check-only` 快速全量验证
3. 用 `cr exec` 隔离验证单个表达式写法

```cirru.cli
; 检查某个定义的代码和内容
calcit.cli/show-def $ {} (:target |ns/def)
calcit.cli/tree-show $ {} (:target |ns/def) (:path |3.1.0)
calcit.cli/search-def $ {} (:target |ns/def) (:keyword |suspect-symbol)
calcit.cli/find-symbol $ {} (:symbol |my-function)
calcit.cli/list-defs $ {} (:namespace |my.namespace)
calcit.cli/show-error $ {}
```

```bash
cr project.cirru exec << 'END'
calcit.cli/show-def $ {} (:target |ns/def)
calcit.cli/tree-show $ {} (:target |ns/def) (:path |3.1.0)
END
```

### 错误信息对照表

| 错误信息                                         | 原因                                              | 解决方法                                                                 |
| ------------------------------------------------ | ------------------------------------------------- | ------------------------------------------------------------------------ |
| `tree-show: invalid path ...`                     | path 含非法段或非数字                             | 重新 `search-def` 获取正确 path，只用点号分隔数字                        |
| `tree-replace: root path is not allowed`         | 写操作传了空 path                                 | 改用 `edit-def` 覆盖整段定义                                             |
| `add-import: import rule already exists`         | 重复添加相同 import                               | 跳过或先手动移除旧规则                                                   |
| `Definition 'xxx' already exists`                | `edit-def` 未传 overwrite                         | 加 `(:overwrite true)`                                                     |
| `tag-match expected tuple`                       | 传入 vector 而非 tuple                            | 改用 `::` 语法，如 `:: :event-name data`                                 |
| `unknown symbol: xxx`                            | 符号未定义或未 import                             | `find-symbol` 确认位置，`add-import` 引入                                |
| `expects pairs in list for let`                  | `let` 绑定语法错误                                | 改为 `let ((x val)) body`（双层括号）                                    |
| `cannot be used as operator`                     | 末尾符号被当作函数调用                            | 改用 `, acc` 前缀传递值，或用函数包裹                                    |
| `unknown data for foldl-shortcut`                | 参数顺序错误（Calcit vs Clojure 差异）            | Calcit 集合在第一位：`map data fn`                                       |
| 字符串被拆分成多个 token                         | 没有用 `\|` 或 `"` 包裹                           | 使用 `\|complete string` 或 `"|complete string"`                         |
| `Type warning` 导致 exec 失败                    | 类型不匹配（阻断执行）                            | 优先检查 `:schema` / `hint-fn` 的参数标注                                  |
| `W_CLI_OPTION_UNKNOWN_KEY`                       | `calcit.cli/*` 选项 key 拼写错误                  | 对照 Options 列表，如 `:file-path` 而非 `:file-pth`                        |
| `W_CLI_OPTION_MISSING_REQUIRED`                  | 缺少必填选项                                      | 补全 map，如 `peek-def` 必须含 `(:target …)`                               |
| `W_CLI_OPTION_TYPE_MISMATCH`                     | 选项值类型错误                                    | `:lines` 用数字；字符串用 `\|…`；布尔用 `true`/`false`                     |
| `cr query error` 无报错但页面仍异常              | 问题不在 Calcit 语义链路，而在 CSS/DOM/业务值     | 到真实运行环境核对渲染结果、属性值和外部依赖                               |

> 💡 **错误文件备份**：`.calcit-error.cirru` 会保存最近一次的完整错误堆栈。直接用 `cat .calcit-error.cirru` 读取，或 `cr query error`（从此文件读取并格式化输出）。
