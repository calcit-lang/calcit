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
  - "calcit exec"
  - "calcit tree replace"
  - "calcit edit def"
---

# Calcit 编程 Agent 指南

本文档为 AI Agent 提供 Calcit 项目的操作指南。

本文定位为 Agents 约束与完整操作手册：覆盖硬前置步骤、命令边界、复杂重构与系统化排障。`docs/CalcitAgent.md` 用于查询与局部编辑速查，不替代本文中的约束规则。

## 🚀 🚀 快速开始与零逃逸 Stdin 工作流（新 LLM 必读）

对于所有接收表达式和代码输入的修改命令（如 `calcit tree replace`, `calcit edit def`, `calcit edit add-import`, `calcit edit schema` 等），有三种输入方式，按推荐顺序：

| 方式                | 适用场景             | 格式检测                     |
| ------------------- | -------------------- | ---------------------------- |
| **stdin**（免参数） | 多行代码、含特殊字符 | 自动（`[`=JSON，其他=Cirru） |
| `--file <path>`     | 从文件读取           | 自动                         |
| `--code <text>`     | 单行简单代码         | 自动                         |

当**同时省略 `--file` 和 `--code`** 时，命令默认从 stdin 读取。无需 Shell 转义，不需临时文件——这是极力推荐的高级重构方式。

确实需要复用或审阅 `--file` 输入时，在项目内使用 `.calcit/snippets/<name>`，并让 `.calcit/` 保持在 `.gitignore`；不要为仓库相关输入使用全局 `/tmp`。CLI 会在 stderr 提示迁移 `/tmp/...` 与 `/private/tmp/...` 路径。

### 统一查询命令

```bash
calcit query ns
calcit query defs app.core
calcit query def 'app.core/main!'
calcit query peek 'app.core/main!'
calcit query search <keyword> --filter 'app.core/main!'
calcit query find main!
calcit query schema 'app.core/main!'
calcit query examples 'app.core/main!'
calcit query config
calcit query path app.main --selector 'path heading def {} :name |add nth 2'
calcit query anchors app.main
calcit config modules
calcit tree show 'app.core/main!' --path @3.1.0
```

### 极力推荐：免参数 Stdin 重构流（在 zsh / bash 下通过 heredoc）

```bash
# 不需要本地临时文件，不需要艰难的 Shell 字符转义，直接传递多行 Cirru 结构定义
calcit calcit.cirru tree replace 'app.main/main!' --path '@3.1' << 'END'
quote (println |abc)
END

# 添加导入 (edit add-import) 也同样天然支持免参数从 stdin 读取
calcit calcit.cirru edit add-import app.main << 'END'
quote (app.config :refer $ dev?)
END
```

> 💡 提示：如果只需做单行短代码段修改，也可以直接使用传统的 `--code 'quote ...'`。

`edit schema` 同样要求 `quote` 边界：原子类型写成 `--code 'quote :string'`，参数化类型写成 `--code 'quote $ :: :ref :bool'`，函数 schema 的 payload 使用 `:: :fn $ {}` 包装。`edit examples` 则要求每个顶层 example 各自带 `quote`，例如 `quote $ add 1 2` 和 `quote |literal`。这样 leaf 与表达式在 CLI 中始终可表示，也不需要额外的 `--leaf` 分支。

---

## 🔧 代码动态运行：`calcit exec`

在仅用于执行或评估外部代码的场景，`calcit exec` 会直接评估通过标准输入 stdin 传入的代码，常用来快速测试独立的 Cirru 代码行为：

```bash
# 也可以用管道传单行代码进行评估
echo 'range 10' | calcit exec
```

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

- **`calcit.cirru` / `compact.cirru`** - 这是 Calcit 程序的运行时快照格式；推荐使用 `calcit.cirru`，旧文件名 `compact.cirru` 仍兼容，必须使用 `calcit edit`/`calcit tree` 进行修改

这两个文件的格式对空格和结构极其敏感，直接文本修改会破坏文件结构。请使用 `calcit query`/`calcit tree`/`calcit edit` 等 CLI 命令进行代码查询和修改。

## Calcit 与 Cirru 的关系

- **Calcit** 是编程语言本身（一门类似 Clojure 的函数式编程语言）
- **Cirru** 是语法格式（缩进风格的 S-expression，类似去掉括号改用缩进的 Lisp）
- **关系**：Calcit 代码使用 Cirru 语法书写和存储

**具体体现：**

- `calcit.cirru`（兼容旧文件名 `compact.cirru`）使用 Cirru 语法存储，必须用 `calcit tree`/`calcit edit` 命令修改
- `calcit cirru` 工具用于 Cirru 语法与 JSON 的转换（帮助理解和生成代码）
- Cirru 语法特点：
  - 用缩进代替括号（类似 Python/YAML）
  - 字符串必须保留 `|` 前缀（如 `|hello`）；含空格时写 `"|hello world"`，双引号本身不表示字符串
  - 单行用空格分隔元素（如 `defn add (a b) (+ a b)`）

**类比理解：**

- Python 语言 ← 使用 → Python 语法
- Calcit 语言 ← 使用 → Cirru 语法

生成 Calcit 代码前，建议先运行 `calcit cirru show-guide` 了解 Cirru 语法规则。

---

## Calcit CLI 命令

详细内容已移入独立文件。以下子命令各有完整文档：

- `主要运行命令` → [run/cli-options.md](./cli-options.md)
- `查询子命令` → [run/query.md](./query.md)
- `文档子命令` → [run/docs-libs.md](./docs-libs.md)
- `Cirru 语法工具` → `calcit cirru show-guide`
- `精细代码树操作` → [run/edit-tree.md](./edit-tree.md)
- `代码编辑` → [run/edit-tree.md](./edit-tree.md)

> 本文档工作流以标准 `calcit` CLI 命令为主。

### LLM 辅助：动态方法提示

在运行时调试 trait 分派时，可使用以下内置函数（低频场景，需运行期有值后调用）：

- `&methods-of value` — 列出某值的可用方法名（返回字符串列表 `[] |.foo |.bar ...`）
- `&inspect-methods value` — 打印方法与 impl 来源（调试 trait override 链，可临时插入 pipeline）
- `impl-origin impl` — 以 `Option<Trait>` 读取 nominal impl 的 trait 来源
- `&trait-call Trait :method receiver & args` — 显式消歧：只调用属于指定 trait 的方法实现

> 📖 深入了解 trait 实现机制：`calcit docs read traits.md` 或 `calcit docs search 'trait-call'`

### 复杂表达式分段组装策略 (Incremental Assembly) ⭐⭐⭐

当需要构造非常复杂的嵌套结构（例如递归循环、多级 `let` 或 `if`）时，直接通过 Shell 传 Cirru 代码容易遇到转义、括号对齐或长度限制等问题。推荐使用**分段占位组装**策略：

简单提示：

- 占位符统一使用 `{{NAME}}` 风格，例如 `{{BODY}}`、`{{TRUE_BRANCH}}`；
- 大表达式可以先用 `calcit query def '<ns/def>'` 看整体；
- 再用 `calcit tree show '<ns/def>' --path @3.1.0` 深入某个片段；
- 真正填充时，优先用 `calcit tree search-replace` 找占位符，不唯一时再退回 `calcit tree replace`。

1. **确立骨架**：先替换目标节点为一个带有占位符的简单结构。

   ```bash
   calcit tree replace '<ns/def>' --path @4.0 --code 'quote (let ((x 1)) {{BODY}})'
   ```

2. **定位占位符**：使用 `tree show` 确认占位符的具体路径。

   ```bash
   calcit tree show '<ns/def>' --path @4.0
   ```

3. **填充内容**：针对占位符路径进行下一层的精细替换。

   ```bash
   calcit tree replace '<ns/def>' --path @4.0.2 --code 'quote (if (= x 1) {{TRUE_BRANCH}} {{FALSE_BRANCH}})'
   ```

4. **递归迭代**：重复上述步骤直到所有占位符都被替换为最终逻辑。

**优势：**

- **精确性**：`--code` 直接传 Cirru one-liner，preprocess 会校验结构。
- **低风险**：每次只修改一小部分，出错时容易通过 `tree show` 快速定位。
- **绕过限制**：解决某些终端对超长命令行参数的限制。

## Calcit 语言基础

详细内容已移入独立文件：

- `Cirru 语法核心概念` → [cirru-syntax.md](../cirru-syntax.md)
- `数据结构：Anonymous Enum vs List` → [features/anonymous-enums.md](../features/anonymous-enums.md)
- `类型标注与检查` → [features/static-analysis.md](../features/static-analysis.md)

### 其他易错点

比较容易犯的错误：

- Calcit 中只有保留 `|` 前缀的 token 才是 string；`|x` 对应 JavaScript 字符串 `"x"`。双引号只负责保护含空格的 token，`"x"` 仍是 symbol。
- Calcit 采用 Cirru 缩进语法，可以理解成去掉跨行括号改用缩进的 Lisp 变种。用 `calcit cirru parse` 和 `calcit cirru format` 互相转化试验。
- Calcit 跟 Clojure 在语义上比较像，但 Cirru 主要用缩进、`$` 和圆括号建立 child list；`[]`、`{}`、`#{}` 是集合构造符号，不是包围内容的 delimiter。

## 开发调试

详细内容已移入 [run/debugging.md](./debugging.md)。包括：

- watcher 监听模式（`calcit -w` / `calcit js -w`）
- 增量触发更新（`calcit edit inc --changed ...`）
- runtime/watcher 持久化错误栈（`calcit query error`）

## 文档支持

遇到疑问时使用：

- `calcit docs search keyword` — 在 `docs/` 与 `~/.config/calcit/docs` 中 grep markdown
- `# see: calcit cirru show-guide` — 读取 Cirru 语法指南
- `calcit docs read path.md` — 读取任意文档文件
- `calcit docs agents [<heading> ...] [--full]` — 结构化 Agent 指南
- `calcit docs read` / `sections` / `remote-libs` 等 — 见 [debugging.md](./debugging.md)

---

## 代码修改示例

### 添加新函数

```bash
calcit edit def 'app.core/multiply' --code 'quote (defn multiply (x y) (* x y))'
```

### 基本操作

```bash
# 添加新函数
calcit edit def 'app.core/multiply' --code 'quote (defn multiply (x y) (* x y))'

# 覆盖已有定义（`--overwrite`）
calcit edit def 'app.core/multiply' --code 'quote (defn multiply (x y) (* x y))' --overwrite

# 添加 :refer import
calcit edit add-import app.main --code 'quote (app.util :refer $ helper)'

# 触发热更新（watcher 模式下写入 .compact-inc.cirru）
calcit edit inc --changed 'app.core/my-fn'
```

### 修改定义工作流

```bash
# 1. 搜索定位（返回 path + leaf-preview）
calcit query search pattern --filter '<ns/def>'

# 2. 查看节点上下文
calcit tree show '<ns/def>' --path @3.1.0

# 3. 执行替换
calcit tree replace '<ns/def>' --path @3.1.0 --code 'quote new-value'

# 4. 或搜索替换叶子
calcit tree search-replace '<ns/def>' --pattern old-sym --code 'quote new-sym'

# 5. 验证写回结果
calcit query def '<ns/def>'
```

---

## 🔧 实战重构场景

以下是开发中最常见的局部修复和重构操作。

### 提取子表达式为新定义（`split-def`）

**场景：** 函数体内某个嵌套子表达式太复杂，想拆成独立的命名定义。

```bash
# 1. 搜索并定位目标子表达式
calcit query search complex-call --filter 'app.core/process-data'

# 2. 查看上下文确认路径
calcit tree show 'app.core/process-data' --path @3.2.1

# 3. 提取为新定义
calcit edit split-def 'app.core/process-data' --path @3.2.1 --name extracted-calc

# 4. 验证结果
calcit query def 'app.core/extracted-calc'
calcit query def 'app.core/process-data'
```

### 重命名定义（`rename-def`）

**场景：** 定义名字需要在同一命名空间内改名。

```bash
# 1. 确认有哪些地方引用到
calcit query usages 'app.core/old-name'

# 2. 搜索所有引用位置
calcit query search old-name --filter 'app.core/caller-fn'

# 3. 重命名定义
calcit edit rename 'app.core/old-name' new-name

# 4. 批量更新引用
calcit tree replace-leaf 'app.core/caller-fn' --pattern old-name --code 'quote new-name'
```

### 迁移定义到另一命名空间（`mv-def`）

**场景：** 某函数放错了命名空间，需要迁移。

```bash
calcit edit mv-def app.util/helper-fn app.core/helper-fn
calcit edit add-import app.main --code 'quote (app.core :refer $ helper-fn)'
```

```bash
# watcher 模式下触发热更新
calcit edit inc --changed 'app.core/helper-fn'
```

### 在定义内移动 / 复制 AST 节点（`edit cp` / `edit mv`）

**场景：** 函数体内某个子表达式需要移到另一位置，或复制用于多处。

```bash
# 定位节点
calcit query search process --filter 'app.core/main-fn'
calcit tree show 'app.core/main-fn' --path @3.1.2

# 复制或移动（position: |before |after |prepend-child |append-child |replace）
calcit edit cp 'app.core/main-fn' --from @3.1.2 --path @3.0 --at after
calcit edit mv 'app.core/main-fn' --from @3.1.2 --path @3.0 --at after
```

### 包裹 / 拆包 / 提升节点（`tree-wrap` / `tree-unwrap` / `tree-raise`）

**场景：** 临时包裹一层 `println` 调试、反向拆掉包装层、或用子节点替换掉父节点。

```bash
# wrap：模板中用 self 引用原节点
calcit tree wrap 'app.core/main-fn' --path @3.1.2 --code 'quote (println self)'

# unwrap / raise
calcit tree unwrap 'app.core/main-fn' --path @3.1.2
calcit tree raise 'app.core/main-fn' --path @3.1.2
```

### 批量重命名局部变量

**场景：** 某函数内某个局部变量名需要统一改掉。

```bash
# 搜索替换所有匹配 of leaf
calcit tree replace-leaf 'app.core/process' --pattern old-var --code 'quote new-var'
```

### 树形展示路径标注（`--path-annotations`）

**场景：** 深层嵌套表达式难以手动数坐标时，让 CLI 自动标注每个 list 节点的路径。

```bash
# 默认行为：纯代码展示
calcit tree show 'app.main/main!' --path '0'

# 开启路径标注：每个嵌套 list 末尾追加 ; "previous node path: X.Y.Z" 注释
calcit tree show 'app.main/main!' --path '0' --path-annotations
```

输出示例：

```cirru
defn process (xs)
  let
      ys $ map xs inc
        ; "previous node path: @3.0.0.1.2"
      ; "previous node path: @3.0.0"
    foldl zs 0 add
    ; "previous node path: 3.2"
  ; "previous node path: 3"
```

> 当节点子节点较多时，底部自动提示可开启 `--path-annotations`。LLM 直接从注释中复制路径即可用于 `--path` 参数。

### 多匹配候选选择（`--pick`）

**场景：** `search-replace` 匹配到多个节点时，用 `--pick <N>` 直接选择而非手动复制路径。

```bash
# 多匹配时列出候选（带路径、上下文、命令建议）
calcit tree search-replace 'app.main/main!' --pattern 'old-name' --code 'quote new-name'

# 直接选择第 2 个候选
calcit tree search-replace 'app.main/main!' --pattern 'old-name' --code 'quote new-name' --pick 2
```

候选展示格式：

```
[0] Path @1.3.0: "old-name"
    Context: defn update $ old-name new-name
    Command: calcit tree search-replace 'app.main/main!' --pattern 'old-name' --code '...' --pick 0

[1] Path @2.5.2: "old-name"
    Context: let $ old-name x $ do-something old-name
    Command: calcit tree search-replace 'app.main/main!' --pattern 'old-name' --code '...' --pick 1
```

### 锚点链式搜索（`--selector`）

**场景：** 用语义路径表达式缩小搜索范围，在特定子树内做搜索替换。

```bash
# 在 add 函数的 body（第 2 个子节点）→ let → bindings（第 0 个子节点）中搜索替换
calcit tree search-replace 'app.main/main!' \
  --selector 'path heading def {} :name |add nth 2 heading let nth 0' \
  --pattern 'old-var' \
  --code 'quote new-var'
```

与 `calcit query path` 使用同一套选择器语法（裸叶子/`heading`/`nth`）。等效于先用 `calcit query path` 获取数字路径再传 `--path`，但一步完成。

### 语义路径查询（`calcit query path`）

**场景：** 用语义描述定位节点，获取数字路径用于后续编辑。

```bash
# 解析语义路径为数字坐标
calcit query path app.main --selector 'path
  heading def {} :name |add
  nth 2
  heading let
  nth 0'

# 输出: 0.2.3.0（可直接用于 -p）
```

选择器：

- 裸叶子 `x`、`|hello`、`42`、`:name` — 当前节点必须是此叶子值
- `heading ...` — 当前 list 的前 N 个子节点匹配（无多余子节点时等同精确匹配）
- `nth N` — 进入当前 list 的第 N 个子节点

### 锚点标注（`calcit query anchors`）

**场景：** 在源码中用 `noted @anchor:<name>` 标记关键位置，查询锚点获取稳定引用。

在源码中标记：

```cirru.no-check
defn main! ()
  noted @anchor:init-state
    let
        state $ load-initial-state
      ; ...
```

查询锚点：

```bash
calcit query anchors app.main
# 输出：
#   @anchor:init-state -> app.main/main! @1
#   @anchor:render-loop -> app.main/main! @4.2
```

> 锚点附着在表达式上，`tree` 编辑操作后自然跟随，比数字路径更稳定。

---

## ⚠️ 常见陷阱和最佳实践

### 1. 路径索引动态变化问题 ⭐⭐⭐

**核心原则：** 删除/插入会改变同级后续节点索引。

**批量修改策略：**

- **从后往前操作**（推荐）：先删大索引，再删小索引
- **单次操作后重新搜索**：每次修改立即用 `calcit query search` 更新路径
- **为旧 path 增加内容护栏**：`tree delete/replace/insert-*` 传入 `--expect 'quote ...'`，节点或锚点不一致时拒绝写入
- **整体重写**：优先用 `calcit edit def --overwrite` 覆盖；根路径 `tree replace` 只保留给明确需要根节点级别改写的场景

非法 path 会抛出明确错误，例如 `tree-show: invalid path 'bad.path': segment 'bad' is not an unsigned integer`。

### 1.5 根路径整体替换的边界 ⭐⭐⭐

`calcit tree replace` 的 path 不能为空（写操作不允许 root path）。当你需要完整替换一个定义体时：

- 更推荐 `calcit edit def ns/def --code 'quote (defn ...)' --overwrite`
- 先在 snippet 里组织完整定义，再一次性覆盖，验证也更直接
- 替换成功后仍应立刻执行 `calcit query def` 确认写回结构符合预期

### 2. 输入格式：Cirru one-liner 字符串 ⭐⭐⭐

`--code` 参数必须是带 `quote` 边界的 **Cirru EDN quoted AST**。`quote` 恰好包住一个节点，写入前会被 CLI 剥离：

- AST path：`calcit tree show app.main/fn --path @3.1.0`
- 表达式：`calcit tree replace app.main/fn --path @2 --code 'quote (println |hello)'`
- symbol leaf：`calcit tree replace app.main/fn --path @2.0 --code 'quote new-symbol'`
- string leaf：`calcit tree search-replace app.main/fn --pattern '|old text' --code 'quote "|new text"'`
- 覆盖已有定义：`calcit edit def app.main/fn --code 'quote (defn fn () nil)' --overwrite`

**实战示例：**

```bash
# ✅ 替换表达式
calcit tree replace app.main/fn --path @2 --code 'quote (println |hello)'

# ✅ 替换 symbol leaf
calcit tree replace app.main/fn --path @2.0 --code 'quote new-symbol'

# ✅ 搜索替换 symbol leaf
calcit tree search-replace app.main/fn --pattern old-var --code 'quote new-var'
```

### 3. Cirru 字符串和数据类型 ⭐⭐

**Cirru 字符串必须带 `|` 前缀：**

```text
|hello            => string "hello"
"|a b c"          => string "a b c"
"|[tag] text"     => string "[tag] text"
hello / "hello"   => symbol hello, not a string
```

双引号只负责把含空格或特殊字符的内容保留为一个 token；它不会单独把 symbol 变成 Calcit string。

**不放心修改是否正确？** 每步后用 `calcit tree show` 验证。

**Anonymous Enum vs List：**

```cirru.no-check
; ✅ anonymous enum - 用于事件、模式匹配
%:: _ :clipboard/read text

; ✅ Vector - 用于 DOM 列表
[] (button) (div)

; ❌ 错误：用 vector 传事件
send-to-component! $ [] :clipboard/read text
; 报错：tag-match expected enum value

; ✅ 正确：用 anonymous enum
send-to-component! $ %:: _ :clipboard/read text
```

**记忆规则：**

- **`%:: _` (anonymous enum)**: 事件、模式匹配、短生命周期 tagged data；`::` 仍是简写
- **`[]` (list)**: DOM 元素列表、动态集合

### 4. 输入大小限制 ⭐⭐⭐

`calcit edit def` 和 `calcit tree replace` 的 code 参数通过 Cirru one-liner 传入，建议单次不超过 **1000 字符**。

**大资源处理建议：**
如果需要修改复杂的长函数，不要尝试一次性替换整个定义。应先构建主体结构，使用占位符，统一写成 `{{PLACEHOLDER_FEATURE}}` 这种花括号形式，并注意避免重复，然后通过 `calcit tree search-replace` 或 `calcit tree replace` 做精准的分段替换。

`calcit query def` / `calcit tree show` 面向 LLM 脚本调用，返回稳定字符串。

### 5. 命名空间 import 操作 ⭐⭐⭐

`calcit edit add-import` **仅支持 `:refer` 单符号导入**：

```bash
# ✅ 正确：添加 :refer import
calcit edit add-import app.main --code 'quote (app.util :refer $ helper)'

# ✅ 分两次添加 :as 和 :refer（Calcit 不支持合并写法）
calcit edit add-import app.main --code 'quote (app.schema :refer $ schema)'
calcit edit add-import app.main --code 'quote (app.schema :refer $ Op)'
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

> `add-import` 不支持 `:as` / `:rename` / 多条规则批量写入。复杂 import 操作需传统 `calcit edit imports` / `calcit edit add-ns`。

### 6. 推荐工作流程

**基本流程（search 快速定位 ⭐⭐⭐）：**

```bash
# 1. 快速定位
calcit query search target --filter '<ns/def>'

# 2. 查看上下文
calcit tree show '<ns/def>' --path @3.1.0

# 3. 执行修改
calcit tree replace '<ns/def>' --path @3.1.0 --code 'quote new-value'

# 4. 验证
calcit query def '<ns/def>'
```

**新手提示：**

- 不知道目标在哪？用 `calcit query search` 快速找到所有匹配
- 想了解代码结构？用 `tree show` 逐层探索
- 需要批量重命名 leaf？用 `search-replace`
- 不确定修改是否正确？每步后用 `tree show` 验证

### 7. 特殊字符与 Stdin 降噪流动 ⭐⭐⭐

Calcit 字段名、函数名常包含 `?`, `->`, `!` 等 Bash 敏感字符，在传统 Shell 中极易发生参数展开错误。使用 Stdin / Heredoc 传入指令或代码时**完全不需要转义**。

### 8. 用 Stdin 传入多条重构动作

我们可以直接利用 Shell 的 heredoc 顺畅地向 `calcit` 写入复杂的内联代码，绝无任何转义焦虑。

---

## 🔄 完整功能开发示例

以下展示从零开始添加新函数的完整流程，是最常见的日常开发场景。

### 步骤 1：确认目标命名空间和现有代码

```bash
# query is done via calcit CLI
calcit query defs app.util
calcit query peek 'app.util/format-date'
calcit query def 'app.util/format-date'
```

### 步骤 2：用 exec 快速验证写法

在真正写入项目前，先用 `calcit exec` 验证逻辑思路：

```bash
calcit project.cirru exec << 'END'
string->number |123
END
```

```bash
calcit project.cirru exec << 'END'
let ((x 10) (y 20)) (+ x y)
END
```

> 💡 有类型警告时 exec 会以错误退出——正好可以提前发现用法错误。

### 步骤 3：添加新定义

```bash
calcit edit def 'app.util/calculate-discount' --code 'quote (defn calculate-discount (price rate) (* price (- 1 rate)))'
calcit query def 'app.util/calculate-discount'
```

### 步骤 4：在调用方添加 import 并使用

```bash
calcit query defs app.core
calcit edit add-import app.core --code 'quote (app.util :refer $ calculate-discount)'
calcit query search total-price --filter 'app.core/checkout'
calcit tree replace 'app.core/checkout' --path @3.2.1 --code 'quote (calculate-discount total-price 0.1)'
```

### 步骤 5：触发热更新并验证

```bash
calcit edit inc --changed 'app.util/calculate-discount' --changed 'app.core/checkout'
calcit query def 'app.util/calculate-discount'
calcit query def 'app.core/checkout'
```

> 可运行 `calcit --check-only` 做全量验证，或 `calcit js` 快速编译。

### 常见失误快速修复

```bash
# 忘记 import → unknown symbol
calcit edit add-import app.core --code 'quote (app.util :refer $ calculate-discount)'

# 函数参数顺序传错 → 定位并修改调用
calcit query search calculate-discount --filter 'app.core/checkout'
calcit tree replace 'app.core/checkout' --path @3.2.1 --code 'quote calculate-discount'
```

> `edit rename` 拼写错误可用 `calcit edit rename` 修正。

---

## 💡 Calcit vs Clojure 关键差异

**语法层面：**

- **定界方式不同**：`[]`、`{}`、`#{}` 是 Calcit 集合构造符号，不是 Clojure/JSON 那样包住内容的 delimiter；Cirru 主要用缩进、`$` 和圆括号建立 child list
- **函数前缀**：Calcit 用 `&` 区分内置函数（`&+`、`&str`）和用户定义函数

**集合函数参数顺序（易错 ⭐⭐⭐）：**

- **Calcit**: 集合在**第一位** → `map data fn` 或 `-> data (map fn)`
- **Clojure**: 函数在第一位 → `map fn data` 或 `->> data $ map fn`
- **症状**：`unknown data for foldl-shortcut` 报错
- **原因**：误用 `->>` 或参数顺序错误

**其他差异：**

- **宏系统**：Calcit 更简洁，缺少 Clojure 的 reader macro（如 `#()`）
- **数据类型**：Calcit 的 Anonymous Enum (`%:: _`，`::` 为简写) 和 List (`[]`) 有不同用途（见"Cirru 字符串和数据类型"）

---

## check-md：验证文档代码块

文档里的 **`cirru` 代码块**会走 `check-md` 的验证路径。

### 运行命令

```bash
# 仓库内验证本文档全部 cirru 块
calcit calcit/test.cirru docs check-md docs/run/agent-advanced.md --entry calcit/test.cirru

# 等价写法（显式 entry）
calcit docs check-md docs/run/agent-advanced.md --entry calcit/test.cirru
```

块类型速查：`cirru` = 完整 eval；`cirru.no-check` = 仅语法示意，不参与类型检查。

### 应通过的案例（纳入 check-md）

下列块使用仓库内真实路径 `calcit/test.cirru`，专门覆盖选项 map 的常见写法：

```bash
# 空 map 即可（:file-path 默认取 calcit entry）
# query is done via calcit CLI
```

```bash
# 必填 + 可选数字（:lines 有默认值，可省略）
# see: calcit query peek app.main/main!
```

```bash
# 多必填 string
# see: calcit query search main --filter app.main/main!
```

```bash
# 无必填项的空 map
# see: calcit cirru show-guide
```

```bash
# bool 选项（:overwrite 默认 false，显式传入）
# see: calcit edit def app.main/reload! --code ...
```

```bash
# 多可选 string（trigger-inc 其余键可省略）
# see: calcit edit inc --changed app.main/main!
```

### 预期失败案例（手动验证，勿放入 `cirru.cli` 块）

`check-md` 要求文档内每个 cirru 块都通过；**故意写错的选项应放在 bash 里用 `calcit eval` 验证**，避免拖垮整份文档的 check-md。

```bash
# 拼写错误 → W_CLI_OPTION_UNKNOWN_KEY
calcit calcit/test.cirru eval '# query is done via calcit CLI (:file-pth |x)'

# 缺少必填 → W_CLI_OPTION_MISSING_REQUIRED（peek-def 缺 :target）
calcit calcit/test.cirru eval 'calcit query peek $ {}'

# 类型错误 → W_CLI_OPTION_TYPE_MISMATCH
calcit calcit/test.cirru eval 'calcit query peek $ {} (:target |app.main/main!) (:lines |bad)'

# 未知 key + 缺必填（可同时报多条告警）
calcit calcit/test.cirru eval 'calcit query peek $ {} (:file-pth |x)'
```

### 选项类型告警码

| 告警码                          | 典型原因                                 | 修复                                                                      |
| ------------------------------- | ---------------------------------------- | ------------------------------------------------------------------------- |
| `W_CLI_OPTION_UNKNOWN_KEY`      | key 拼写错误（如 `:file-pth`）           | 对照函数文档或 `calcit query peek calcit query ns` 的 Options                     |
| `W_CLI_OPTION_MISSING_REQUIRED` | 未传必填项（如 `peek-def` 缺 `:target`） | 补全 map 中的必填 tag                                                     |
| `W_CLI_OPTION_TYPE_MISMATCH`    | 值类型不符（如 `:lines` 收到 string）     | string 按上文规则；number 用数字；bool 用 `true`/`false`                  |

统一使用标准 `calcit` CLI 命令。

---

## 常见错误排查

### 快速诊断流程

当 watcher 提示有错误或行为异常时，按以下顺序排查：

1. 先保留当前失败命令的 stderr；需要最近 runtime/watcher stack 时再运行 `calcit query error`，若提示 stale 则忽略旧栈
2. 用 `calcit --check-only` 快速全量验证
3. 用 `calcit exec` 隔离验证单个表达式写法

```bash
# 检查某个定义的代码和内容
calcit query def ns/def
calcit tree show ns/def --path @3.1.0
calcit query search suspect-symbol --filter ns/def
calcit query find my-function
calcit query defs my.namespace
calcit query error
```

`calcit exec` 的 stdin 是待求值的 Calcit 源码，不是 shell 命令流；`calcit query`、`calcit tree` 等 CLI 命令应像上面那样分别执行。

### 错误信息对照表

| 错误信息                                 | 原因                                          | 解决方法                                               |
| ---------------------------------------- | --------------------------------------------- | ------------------------------------------------------ |
| `tree-show: invalid path ...`            | path 含非法段或非数字                         | 重新 `calcit query search` 获取正确 path，只用点号分隔数字 |
| `tree-replace: root path is not allowed` | 写操作传了空 path                             | 改用 `calcit edit def --overwrite` 覆盖整段定义            |
| `add-import: import rule already exists` | 重复添加相同 import                           | 跳过或先手动移除旧规则                                 |
| `Definition 'xxx' already exists`        | `calcit edit def` 未传 `--overwrite`              | 加 `--overwrite`                                       |
| `tag-match expected enum value`          | 传入 list 而非 enum                           | 改用 `%:: _`，如 `%:: _ :event-name data`               |
| `unknown symbol: xxx`                    | 符号未定义或未 import                         | `calcit query find` 确认位置，`calcit edit add-import` 引入    |
| `expects pairs in list for let`          | `let` 绑定语法错误                            | 改为 `let ((x val)) body`（双层括号）                  |
| `cannot be used as operator`             | 末尾符号被当作函数调用                        | 改用 `, acc` 前缀传递值，或用函数包裹                  |
| `unknown data for foldl-shortcut`        | 参数顺序错误（Calcit vs Clojure 差异）        | Calcit 集合在第一位：`map data fn`                     |
| 字符串被拆分成多个 token                 | 含空格字符串没有保留为一个 string token       | 使用上文的 spaced-string 写法                          |
| `Type warning` 导致 exec 失败            | 类型不匹配（阻断执行）                        | 优先检查 `:schema` / `hint-fn` 的参数标注              |
| `W_CLI_OPTION_UNKNOWN_KEY`               | 选项 key 拼写错误                             | 对照 Options 列表，如 `:file-path` 而非 `:file-pth`    |
| `W_CLI_OPTION_MISSING_REQUIRED`          | 缺少必填选项                                  | 补全 map，如 `peek-def` 必须含 `(:target …)`           |
| `W_CLI_OPTION_TYPE_MISMATCH`             | 选项值类型错误                                | `:lines` 用数字；字符串按上文规则；布尔用 `true`/`false` |
| `calcit query error` 无报错但页面仍异常      | sidecar 陈旧，或问题在 CSS/DOM/业务值等外部链路 | 先看 stale 提示与当前 stderr，再到真实运行环境核对     |

> 💡 **错误文件备份**：`.calcit/error.cirru` 保存最近一次被持久化的错误堆栈，不保证每个失败命令都刷新。可用 `calcit query error` 格式化读取；看到 stale 提示时，以当前命令 stderr 为准。
