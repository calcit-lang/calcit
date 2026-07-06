# RFC: 语义化树形导航与编辑

状态：Draft  
日期：2026-07-06  
关联：`cr tree search-replace`、`cr tree show`、`cr query search`、`03-18-query-def-tree-show-chunked-display-plan.md`

---

## 1. 概要

当前 `cr tree` 系列编辑命令在定位子表达式时依赖纯数字点号路径（如 `-p '0.3.2.1'`）。对于人类而言手动数坐标已经不方便，对于 LLM 而言更是结构性难题——LLM 在精确计数方面的可靠性与人类手动数行号相当。

本 RFC 提出 **四层互补方案**，从近到远逐步提升编辑体验：

| 层级   | 方案                | 技术路径                                                                              |
| ------ | ------------------- | ------------------------------------------------------------------------------------- |
| **L1** | 展示时自动标注路径  | `tree show` 输出中为每个 list 表达式末尾追加 `; "previous node path: 1.2.3"` 注释节点 |
| **L2** | 多候选交互确认      | `search-replace` 多匹配时列出候选而非报错                                             |
| **L3** | 锚点 + 相对偏移搜索 | `search-replace --at ... --child N` 先定位父节点再做子节点替换                        |
| **L4** | 结构化查询语言      | 语义路径表达式：`path` 开头，裸叶子 + `heading` + `nth` 三步导航                      |

核心设计原则：

- **所有查询表达式均使用 Cirru 语法**（无 Lisp 风格外层括号）
- **路径表达式中叶子字面量直接表示严格匹配**，表达式取首 token 作为语义算子（`heading` / `nth`）
- **路径注释使用 `; "previous node path: 1.2.3"` 格式**，作为 list 末尾的注释节点追加，不改变已有子节点索引
- **锚点使用已有 `calcit.core/noted` macro**，格式为 `noted @anchor:<name> expr`

---

## 2. 动机

### 2.1 现状问题

当前 LLM 编辑 Calcit 代码的标准工作流：

```
1. cr tree show 'app.main/main!'             → 查看代码结构
2. LLM 自己数目标表达式的坐标                   → 容易数错
3. cr tree replace 'app.main/main!' -p '...'  → 可能用错路径
4. 出错后重新数、重新试                          → 迭代成本高
```

或者用 `search-replace`：

```
1. cr tree search-replace 'app.main/main!' --pattern 'old-expr' ...
   → 报错："Found 3 matches"
2. LLM 需要切回 tree show 手动辨别是哪个匹配
3. 回到手动数坐标模式
```

### 2.2 目标工作流

```
1. cr tree show 'app.main/main!'  → 输出自动带路径注释
2. LLM 直接从注释中复制路径          → 不需要数
3. cr tree replace 'app.main/main!' -p '复制来的路径' ...
   → 一次成功
```

或者在多匹配场景：

```
1. cr tree search-replace ... --pattern '...'
   → 列出 3 个候选（带路径和上下文）
2. LLM 用 --pick 0 或 --path 指定
   → 一次成功
```

---

## 3. L1：展示时自动标注路径

### 3.1 方案

在 `cr tree show` 的输出中，为每个 **list 节点**的末尾追加一条路径注释。注释放在末尾而非开头，避免插入前导节点导致已有子节点索引偏移。格式为 Cirru 行注释语法：

```cirru
defn add (a b)
  &+ a b (; "previous node path: 3.2")
  ; "previous node path: 3"
```

实现方式：**在 AST 层面操作**，为每个 `Cirru::List` 的 children 末尾 `push` 一个 comment 节点——即 `Cirru::List`，其首子节点为 `Cirru::Leaf(";")`，后续子节点为注释内容（如 `Cirru::Leaf("previous node path: 1.2.3")`）。格式化时该 list 自然渲染为行注释 `; "previous node path: 1.2.3"`。

### 3.2 命令

```bash
# 默认行为：纯代码展示，无路径注释
cr tree show 'app.main/main!'

# 开启路径标注（所有嵌套层级末尾标注路径）
cr tree show 'app.main/main!' --path-annotations
```

当展示的节点包含较多子节点（如超过阈值）时，在输出底部提示可用选项：

```
Tip: This node has 15 children. Use --path-annotations to annotate each child
     with its path index for easier editing. Use --chunked to split large
     subtrees into fragments.
```

### 3.3 输出示例

对于如下源码：

```cirru
defn process (xs)
  let
      ys $ map xs inc
      zs $ filter ys even?
    foldl zs 0 add
```

默认 `cr tree show` 输出（无标注，保持旧行为）：

```cirru
defn process (xs)
  let
      ys $ map xs inc
      zs $ filter ys even?
    foldl zs 0 add
```

`--path-annotations` 时（每个嵌套 list 末尾都追加路径注释）：

```cirru
defn process (xs)
  let
      ys $ map xs inc
        ; "previous node path: 3.0.0.1.2"
      ; "previous node path: 3.0.0"
      zs $ filter ys even?
        ; "previous node path: 3.0.1.1.2"
      ; "previous node path: 3.0.1"
    foldl zs 0 add
    ; "previous node path: 3.2"
  ; "previous node path: 3.3"
```

### 3.4 实现要点

- **AST 层面操作**：调用 `children.push(Cirru::List([Cirru::Leaf(";"), Cirru::Leaf(path_string)]))` 追加注释节点，而非字符串拼接
- 注释节点格式：`Cirru::List` 首子节点为 `Cirru::Leaf(";")`，后续为内容叶子（如 `"previous node path: 1.2.3"`），渲染为 `; "previous node path: 1.2.3"`
- `--path-annotations`：递归为所有嵌套 list 末尾追加注释（flag，无参数）
- 默认不追加任何注释节点，保持旧行为
- 当展示的节点子节点较多时，底部输出 tip 提示可开启 `--path-annotations` 或 `--chunked`
- 注释中的路径数字为相对于当前 `-p` 定位 path 的索引
- 使用 dimmed 颜色渲染注释行，不干扰代码阅读
- 根节点的 path 为空字符串, 不用显示
- **末尾追加不改变索引**：注释节点是最后一个 child，不影响已有子节点的相对位置

### 3.5 与 chunked display 的关系

`--path-annotations` 与 `--chunked` 可组合使用，两者独立运作：

- `--chunked`：表达式过大时拆分展示，便于人类阅读整体结构
- `--path-annotations`：在 chunk 内部或普通展示中标注每个节点的路径坐标

同时启用时：先分片，再在每个 fragment 内部标注路径注释。默认两个都不启用。

---

## 4. L2：多候选交互确认

### 4.1 方案

当 `search-replace` 遇到多个匹配时，**不直接报错退出**，而是：

1. 列出所有候选匹配（带路径、上下文预览、序号）
2. 允许用户/LLM 通过 `--pick <index>` 或 `--path <path>` 精确指定
3. 默认行为（无 `--pick` 也无 `--path`）保持不变：报错并要求指定

### 4.2 命令

```bash
# 多匹配时列出候选
cr tree search-replace 'app.main/main!' \
  --pattern 'old-name' \
  --code 'new-name'

# 输出候选列表后，选择第 2 个候选
cr tree search-replace 'app.main/main!' \
  --pattern 'old-name' \
  --code 'new-name' \
  --pick 2

# 或直接用路径指定
cr tree search-replace 'app.main/main!' \
  --pattern 'old-name' \
  --code 'new-name' \
  --at '1.3.0'
```

### 4.3 候选展示格式

当匹配数 > 1 且未指定 `--pick`/`--at` 时：

```
Found 3 matches for pattern "old-name":

[0] Path [1.3.0]: "old-name"
    Context: defn update $ old-name new-name
    Command: cr tree search-replace 'app.main/main!' --pattern 'old-name' ... --pick 0

[1] Path [2.5.2]: "old-name"
    Context: let $ old-name x $ do-something old-name
    Command: cr tree search-replace 'app.main/main!' --pattern 'old-name' ... --pick 1

[2] Path [3.0.1]: "old-name"
    Context: cond $ = old-name nil $ handle-nil old-name
    Command: cr tree search-replace 'app.main/main!' --pattern 'old-name' ... --pick 2

Use --pick <index> to select a candidate, or --at '<path>' to specify directly.
```

### 4.4 实现要点

- `--pick` 和 `--at` 互斥，同时指定时报错
- 候选按路径深度优先排序（与当前遍历顺序一致）
- `--pick` 从 0 开始
- 最多展示 20 个候选，超出部分显示 `... and N more`
- 该行为同样适用于 `search-replace` 的 list-node 匹配（非仅 leaf）

---

## 5. L3：锚点搜索替换（`search-replace --at`）

### 5.1 方案

扩展 `search-replace`，允许先通过内容匹配定位一个**父节点（锚点）**，然后在锚点的第 N 个子节点中做替换。这样 LLM 只需描述"在哪个定义/哪个 let 里面改"，不需要知道锚点的全局坐标。

### 5.2 命令

```bash
# 基本形式：在匹配 anchor 的节点的第 N 个子节点中做搜索替换
cr tree search-replace 'app.main/main!' \
  --at 'defn add' \
  --child 2 \
  --pattern 'old-call' \
  --code 'new-call'

# 多层锚定：在 anchor 内的第 N 个子节点中再锚定
cr tree search-replace 'app.main/main!' \
  --at 'let' \
  --child 0 \
  --at 'cond' \
  --child 1 \
  --pattern 'old-branch' \
  --code 'new-branch'
```

### 5.3 语义

`--at` 与 `--child` 的语义：

- `--at <quoted-code>`：在当前范围内搜索匹配该内容的节点。相当于先做 `search-replace` 的匹配逻辑，**但不替换**。
- `--child <N>`：从匹配到的锚点进入第 N 个子节点，缩小搜索范围。

组合效果：

```
search-replace target --at A --child 0 --at B --child 1 --pattern P --code R
```

等价于：

1. 在 target 中搜索匹配 A 的节点 → 锚点 a
2. 取 a 的第 0 个子节点 → scope₁
3. 在 scope₁ 中搜索匹配 B 的节点 → 锚点 b
4. 取 b 的第 1 个子节点 → scope₂
5. 在 scope₂ 中搜索匹配 P 的节点 → 替换为 R

### 5.4 约束

- `--at` 在目标范围内**必须唯一匹配**，否则报错列出候选（沿用 L2 的交互逻辑）
- `--child` 索引从 0 开始
- 多个 `--at` / `--child` 按出现顺序链式执行
- `--at` 的参数使用与 L4 路径表达式相同的语义：裸叶子表示严格匹配叶子，表达式取首 token 作为算子（见 §6）

---

## 6. L4：结构化查询语言（Cirru Path Expression）

### 6.1 设计目标

提供一种**完全用 Cirru 语法书写**的语义路径表达式，替代纯数字坐标，让 LLM 和人类都能直观描述"我要编辑哪个节点"。

核心设计：

- 表达式以 `path` 开头
- **叶子字面量直接表示严格匹配**：`x`、`|hello`、`42` 等裸叶子值表示"当前节点必须是这个叶子"
- **表达式取首 token 作为语义算子**：`heading`、`nth`
- 从左到右链式执行，每个选择器在上一个匹配结果上继续缩小范围

选择器只有三种：裸叶子精确匹配叶子值，`heading` 按前缀匹配 list 节点，`nth` 按索引进入子节点。配合 L1 的路径标注（`--path-annotations`）显示各子节点索引，无需额外的搜索型选择器。

### 6.2 选择器类型

#### 6.2.1 叶子严格匹配（裸字面量）

叶子节点直接书写，表示"当前节点必须精确等于此值"：

```cirru
path defn add
```

语义：先匹配叶子 `defn`，再匹配叶子 `add`。等价于在 AST 中找连续两个叶子 `defn` `add` 的位置。

- 标识符：`x`、`defn`、`add`
- 字符串：`|hello`
- 数字：`42`
- tag：`:name`

#### 6.2.2 `heading` — 表达式前缀匹配

唯一的 list 节点内容匹配选择器：

```cirru
path
  heading def {} :name |add
```

语义：匹配任何 **前 N 个子节点** 与给定模式一致的 list 节点，允许后面有更多子节点。当无多余子节点时等同于精确匹配。

示例：

```cirru
; 匹配任意以 def 开头的表达式（def, defn, defrecord, defenum ...）
path
  heading def

; 匹配 defn add (a b) (&+ a b)——有 or 没有多余子节点都命中
path
  heading defn add (a b)
    &+ a b
```

嵌套表达式内的 `(a b)` 递归地用相同规则匹配。

#### 6.2.3 `nth` — 位置导航

```cirru
path
  heading defn add (a b)
    &+ a b
  nth 2
```

语义：匹配目标后，进入其第 2 个子节点。

- `nth N`：进入当前匹配节点的第 N 个子节点（从 0 计数）

### 6.3 组合示例

定位 add 函数的第一个 let 绑定：

```cirru
path
  heading def {} :name |add
  nth 2
  heading let
  nth 0
```

等价于：

1. 匹配任意 `def {} :name |add ...` 表达式 → 定位 add 函数
2. 进入第 2 个子节点（body）
3. 匹配以 `let` 开头的表达式
4. 进入第 0 个子节点（bindings）

### 6.4 完整语法规范

```
path-expr     = "path" selector*

selector      = leaf-literal            ; 叶子严格匹配
              | list-selector           ; 表达式语义选择器

list-selector = "heading" children  ; 表达式前缀匹配（含精确匹配）
              | "nth" integer        ; 位置导航

children      = (leaf-literal | nested-expr)*
nested-expr   = "(" children ")"        ; 在 Cirru 中即缩进嵌套的 list

leaf-literal  = identifier | string-literal | number | tag
integer        = /\d+/
```

### 6.5 使用场景

#### 场景 A：定位定义

```bash
# 搜索 add 函数的 def，拿到路径
cr query path 'app.main' \
  --selector 'path
    heading def {} :name |add'

# 输出: 0 (def 的顶层索引)
```

#### 场景 B：编辑深层子表达式

```bash
# 在 add 函数的第一个 let 绑定中搜索并替换
cr tree search-replace 'app.main/main!' \
  --path-selector 'path
    heading def {} :name |add
    nth 2
    heading let
    nth 0' \
  --pattern 'old-var' \
  --code 'new-var'
```

#### 场景 C：批量脚本

```bash
# 获取路径后用于后续编辑
PATH=$(cr query path 'app.main' --selector 'path heading def {} :name |init-fn $ nth 2')
cr tree replace 'app.main/main!' -p "$PATH" --code '...'
```

### 6.6 与现有路径的互操作

- `cr query path` 输出标准数字路径（如 `1.3.0`），可直接用于 `-p`
- `--path-selector` 是 `-p` 的超集替代，内部先解析为数字路径再执行
- 解析失败时给出明确错误信息（哪一步匹配失败、已匹配到的范围、剩余选择器是什么）

### 6.7 选择器语义对比

| 选择器        | 匹配方式                        | 示例                   |
| ------------- | ------------------------------- | ---------------------- |
| 裸叶子 `x`    | 当前节点必须是叶子 `x`          | `path defn add`        |
| `heading ...` | 当前 list 的前 N 个子节点匹配   | `heading def {} :name` |
| `nth N`       | 导航到当前 list 的第 N 个子节点 | `nth 2`                |

---

## 7. 锚点注释（Source Annotations）

### 7.1 方案

使用已有的 `calcit.core/noted` macro 在源码中定义**命名锚点**，作为编辑时的稳定引用：

```cirru
defn main! ()
  noted @anchor:init-state
    let
        state $ load-initial-state
      ; ...
```

`noted` 是已有 macro，接受 tag 和表达式两个参数。`@anchor:<name>` 作为 tag 标记该表达式，`noted` 在运行时透传表达式的值，锚点信息不参与运行时语义。

锚点附着在表达式上，表达式被移动/复制时锚点跟随。`cr query anchors` 遍历 AST 中所有 `noted` 调用，提取 `@anchor:` 前缀的 tag 及其路径。

### 7.2 命令

```bash
# 列出所有锚点
cr query anchors 'app.main'

# 输出：
#   @anchor:init-state → app.main/main! [1]
#   @anchor:render-loop → app.main/main! [4.2]

# 用锚点定位
cr tree show 'app.main/main!' --anchor 'init-state'

# 用锚点编辑：在锚点后插入
cr tree insert-after 'app.main/main!' \
  --anchor 'init-state' \
  --code 'println |loaded'
```

### 7.3 约束

- 锚点 tag 以 `@anchor:` 前缀标识，在同一 namespace 内必须唯一（不唯一时报错）
- `noted` 在运行时透传表达式值，锚点不参与运行时语义
- 锚点跟随表达式移动：`tree delete` / `tree insert` 等操作后，锚点随 `noted` 节点自然位移
- `cr query anchors` 遍历 AST 中所有 `noted` 调用，提取路径和名称

### 7.4 锚点与路径的对比

| 特性       | 路径 (`-p '1.3.0'`)    | 锚点 (`--anchor 'init-state'`) |
| ---------- | ---------------------- | ------------------------------ |
| 稳定性     | 编辑后可能失效         | 跟随代码移动，基本稳定         |
| 可读性     | 无意义数字             | 语义化名称                     |
| 设置成本   | 零（自动标注）         | 需要手动添加注释               |
| LLM 友好度 | 低（需要数坐标或复制） | 高（语义化引用）               |

---

## 8. 实施路线

### Phase 1（最小可行）：L1 + L2

- `tree show --path-annotations`：opt-in 标志，开启后所有嵌套层级标注路径
- 大节点底部 tip 提示可开启标注或分片
- `search-replace` 多匹配候选展示（不改默认行为，仅在匹配 > 1 时改进输出）

**预计工作量**：~2-3 天  
**收益**：LLM 可直接从 show 输出复制路径，多匹配时给出可操作建议

### Phase 2：L3 锚点搜索

- `search-replace --at` / `--child` 链式锚定
- 将 `search-replace` 从仅 leaf 匹配扩展到 list-node 匹配

**预计工作量**：~3-5 天  
**收益**：LLM 可用语义描述定位，不再依赖数字路径

### Phase 3：L4 结构化查询语言

- `path` 选择器解析器
- `cr query path` 命令
- `--path-selector` 替代 `-p` 的编辑命令集成

**预计工作量**：~5-7 天  
**收益**：完整的语义化树形查询能力

### Phase 4：锚点注释

- `noted @anchor:<name>` 的识别与提取
- `cr query anchors` 命令
- `--anchor` 参数集成到编辑命令

**预计工作量**：~4-6 天  
**收益**：跨编辑会话的稳定引用

---

## 9. 兼容性

- 所有新参数均为 **opt-in**，现有行为完全保留
- `--path-annotations` 是 flag，默认关闭，传即开启
- `--chunked` 默认关闭，需手动开启
- `--pick` 和 `--path-selector` 与现有 `-p` 互斥
- 锚点使用已有 `noted` macro，不引入新语法，对现有解析无影响

---

## 10. 开放问题

1. **`--path-annotations` 是否应该默认开启？**
   - 关闭（当前决定）：保持旧行为，大节点时底部 tip 引导开启
   - 默认开启：对 LLM 更友好，但改变默认输出
   - 建议默认关闭 + tip 引导，后续根据使用反馈决定是否改为默认开启

2. **`heading` 是否总是够用？**
   - `heading` 匹配前 N 个子节点，无多余子节点时等同于精确匹配
   - 裸叶子处理单值精确匹配
   - 不提供独立的 `exact` 选择器——链式导航模型中没有它的位置
   - 后续评估是否需要 `ends-with`、`contains` 等变体

3. **是否需要支持通配符叶子？**
   - 例：用 `_` 匹配任意单个叶子，`...` 匹配任意剩余子节点
   - `path heading def _ :name` 匹配任意名称的 def
   - 当前暂不支持，可作为后续增强

4. **锚点应缓存还是每次遍历 AST？**
   - 缓存：`cr query anchors` 首次解析后缓存到 snapshot 元数据，编辑后失效重算
   - 实时遍历：更简单，无需维护缓存一致性
   - 由于 `noted` 节点在 AST 中自然存在，遍历成本可控，建议先实时遍历
