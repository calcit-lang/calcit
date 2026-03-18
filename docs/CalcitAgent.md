# Calcit Agent 快速实践（局部查看与编辑优先）

本文档面向 Agent/LLM 的高频工作流，目标是**更快定位、最小改动、低噪音验证**。

## Cirru 语法速览（先看这个）

结构化编辑依赖“树 + 路径”。先能读懂 Cirru，才能稳定算出路径坐标。

- Cirru 是缩进风格的 S-expression，缩进层级就是树层级。
- 行内空格分隔节点；嵌套表达式是子节点。
- 常见字面量：
  - `|text` 或 `"|text"`：字符串, 两者等价, 区别是后者能处理好空格. 其中 `"|t"` 在老代码也会用 "\"t" 写.
  - `:tag`：tag
  - `[]` / `{}`：集合构造
- 你在 `cr query search` 里看到的 `[5.5.1.3]`，本质是“第 5 个子节点的第 5 个子节点的第 1 个子节点的第 3 个子节点”。

### 坐标如何从代码中读出来

示例表达式（简化）：

```cirru
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

```cirru
; "写法 A"
result $ collect! state

; "等价写法 B"
result (collect! state)
```

- 当你把一段调用改成/改掉 `$` 形式时，命中节点的路径经常会变深或变浅。
- 经验：改 `$` 之后，不复用旧路径，重新 `query search` 一次。

#### `,`：在“重起一行”场景里用于保持目标节点形态（有助于坐标稳定）

`,` 常用于告诉解析器“这里是值节点，不是再发起一次调用”。

```cirru
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

#### 实操规则（最稳）

凡是改到 `$` 或 `,`（尤其是从单行改成多行）时：

1. 先 `tree show` 看当前子树。
2. 修改后立刻 `query search <keyword> -f <ns/def>` 重拿路径。
3. 再继续下一步结构化编辑（`replace/wrap/rewrite`）。

## 0) 硬前置步骤

在任何 `cr edit` / `cr tree` 修改前，先执行一次：

```bash
cr docs agents --full
```

这一步不是建议项，用于避免沿用旧命令心智模型。

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
  - 默认最多一条（快速扫读）
  - 支持“全部/静默”模式切换（建议使用 `--tips-level` 统一控制）

> 说明：当前 CLI 已支持 `--no-tips`。`--tips-level` 作为统一分级开关建议保留在后续实现中。

---

## 2) 5 步最小模板（看大表达式并可编辑）

1. 定位目标定义：`cr query defs <ns>`
2. 先轻看再全看：`cr query peek <ns/def>`，必要时再 `cr query def <ns/def>`
3. 搜关键词拿路径：`cr query search <keyword> -f <ns/def>`
4. 聚焦子树确认上下文：`cr tree show <ns/def> -p '<path>'`（复杂时加 `-j`）
5. 修改并验证：`cr tree replace ...` 或 `cr edit inc --changed <ns/def>`，然后 `cr js`

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
- `cr tree show <ns/def> -p '<path>'`：查看局部子树。

### 编辑

- `cr tree replace <ns/def> -p '<path>' -e '<code>'`：替换指定节点。
- `cr tree target-replace <ns/def> --pattern '<leaf>' -e '<code>' --leaf`：按内容唯一定位替换（优先）。
- `cr edit inc --changed <ns/def>`：增量编译当前修改定义。

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

- `cr js`：快速验证当前改动可编译。
- 全量语义回归建议：`yarn check-all`。

---

## 4) 降噪与可读性建议

- 默认只看 Cirru，**必要时**才加 `-j`。
- 先 `query def` 看大轮廓，再 `search` + `tree show` 看局部。
- 搜索结果过多时，不要连续盲改路径；每次改后重搜一次更稳。
- 复杂多行表达式优先 `-f <file>`，减少 shell 转义错误。
- 若需要最安静输出，可使用 `--no-tips`。

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
cr --no-tips query peek <ns/def>

# 2) 必要时才看完整定义（默认 Cirru）
cr --no-tips query def <ns/def>

# 3) 用 search 定位后再 show 局部
cr --no-tips query search '<keyword>' -f <ns/def>
cr --no-tips tree show <ns/def> -p '<path>'
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

## 7) 进阶内容（已下沉）

本文件只保留高频流程。低频/进阶内容请查：

- 完整进阶版 Agent 指南（从旧版完整迁移）：`docs/run/agent-advanced.md`
- 运行模式、eval 细节、CLI 约束：`Agents.md`
- 语言手册与章节阅读：`cr docs list` / `cr docs read <file>`
- Cirru 语法细节：`cr cirru show-guide`
- traits 与运行期方法调试：`cr docs search 'trait-call'`

---

## 8) 一句话原则

**先定位路径，再看子树，再最小替换；默认 Cirru，JSON 只在必要时启用。**

---

## 9) `cr` 能力地图（粗粒度）

当当前模板不够用时，按下面的“能力分层”自行扩展：

- 运行与编译：`cr`, `cr js`, `cr ir`, `-w/--watch`
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
