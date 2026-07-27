# RFC: `cr analyze effects-graph` — State / Transform / Effect 分解图

状态：Draft  
日期：2026-06-15  
关联：`call_tree.rs`、`analyze check-types`、`03-05-function-schema-dual-track-rfc.md`、`02-18-language-theory-evolution-plan.md`

---

## 1. 概要

新增分析命令：

```bash
cr analyze effects-graph [--root ns/def] [options...]
```

从**入口定义**（默认 `:init-fn`）出发，把程序（及其可达子函数）分解为三类信息，并输出一张可递归展开的分析图：

| 维度 | 含义 | 典型内容 |
|------|------|----------|
| **State** | 数据与可变状态 | 参数、返回值、局部绑定、atom/ref、结构体字段流、`d!` 更新槽 |
| **Transform** | 纯逻辑骨架（隐藏实现细节） | 签名、控制流轮廓、调用关系、可压缩的表达式摘要 |
| **Effect** | 除 state 端口外的副作用 | IO、控制台、异常、环境、宿主 API、渲染、异步、watch 回调 |

设计目标：**读图即可理解程序**，而不必展开完整 Cirru 源码。  
同一抽象可作用于整个程序，也可作用于任意单个 `ns/def` 节点（递归分解）。

---

## 2. 动机

### 2.1 现有 `analyze` 能力的缺口

| 已有命令 | 提供的信息 | 不足 |
|----------|------------|------|
| `call-graph` | 定义间**调用结构** | 不区分数据流与副作用；节点仍是“函数名 + doc” |
| `count-calls` | 调用频次 | 无语义分类 |
| `program-diff` | 版本间结构 diff | 不解释运行时语义 |
| `check-types` / `weak-types` | 类型覆盖与薄弱点 | 不组织为可导航的“程序理解图” |
| `query def` / `tree show` | 完整代码 | 细节过多，不适合宏观把握 |

Agent / 人类在理解 Respo 类 UI 程序时，真正关心的是：

- 哪些**状态槽**在流动（`states`、`*abort-control`、`message-box-state`）？
- 哪些函数在做**纯变换**（`map`、`filter`、格式化）？
- 哪些调用会**触达外部世界**（`render!`、`read-file`、`js/...`、`send-to-component!`）？

`effects-graph` 把这些维度显式化。

### 2.2 与“隐藏代码细节”的关系

Transform 不等于删除逻辑，而是**降采样**：

- 保留：分支结构、循环/折叠模式、关键调用边、类型签名
- 省略：字面量、局部临时变量名、宏展开后的噪声
- 可配置：`--detail full|summary|minimal`（默认 `summary`）

类比：`call-graph` 是**结构邻接图**；`effects-graph` 是**语义分解图**。

---

## 3. 核心概念

### 3.1 State

**定义**：在求值过程中可被命名、传递、持久化或观测的数据。

分类（`state.kind`）：

| kind | 说明 | 检测线索 |
|------|------|----------|
| `param` | 函数入参 | `defn` 参数列表 + schema `:args` |
| `return` | 函数返回值 | schema `:return`、尾表达式类型 |
| `local` | 局部绑定 | `let` / `&let` 绑定名 + 推断类型 |
| `atom` | 可变引用单元 | `defatom`、`atom`、`reset!`、`swap!` |
| `field` | 记录/map 字段流 | `assoc`、`get`、`%{}` 构造、`:key` 读写 |
| `import` | 跨 ns 引入的符号 | `:require` / `:refer` 解析结果 |
| `slot` | 框架约定状态槽 | Respo `states`、`d!` 第一参数模式 |

**State 端口（port）**：带方向的类型化槽位，形如：

```json
{
  "id": "app.comp.container/message-box-state:content",
  "kind": "field",
  "type": ":string",
  "direction": "inout",
  "source": "app.comp.container/comp-message-box:3.2"
}
```

`direction`：`in` | `out` | `inout` | `persist`（跨 reload 保留，如 atom）。

### 3.2 Transform

**定义**：在 state 端口之间建立映射的**可命名逻辑单元**，实现细节在输出中默认折叠。

一个 Transform 节点包含：

- `signature`：来自 `CodeEntry.:schema` 或 `hint-fn` 补全结果
- `control`：压缩后的控制流骨架（`if` / `cond` / `match` / `foldl` / `let` 深度）
- `calls`：调用的其他 Transform（指向子节点或外部 `fqn`）
- `summary`：一句话摘要（可选，来自 `:doc` 首行或规则生成）

示例（概念输出）：

```text
└── app.comp.container/comp-message-box  [transform]
    ├── state.in  message-box-state (:record)
    ├── state.out message-box-state (:record)
    ├── control   let → if focus-mode? → textarea | compact-div
    ├── calls     respo.core/textarea, respo.core/div, calcit.core/assoc
    └── effects   (none in pure branch)
```

**宏展开策略**：分析期展开一层；展开结果并入 Transform，不单独暴露宏内部符号（与 `call-graph` 跳过 macro 体一致）。

### 3.3 Effect

**定义**：改变外部世界、控制流、可观测性，或引入**非局部**语义的运算；**不把**已在 State 端口建模的纯数据传递算作 effect。

分类（`effect.kind`，第一版内置表）：

| kind | 示例 | 类型标记 |
|------|------|----------|
| `console` | `println`, `eprintln`（若可静态识别） | `:effect/console` |
| `io/read` | `read-file` | builtin schema |
| `io/write` | `write-file` | builtin schema |
| `env` | `get-env` | builtin schema |
| `control/raise` | `raise` | builtin schema |
| `control/quit` | `quit!` | builtin schema |
| `async` | `hint-fn $ {} (:async true)` 作用域 | `:effect/async` |
| `state/watch` | `add-watch` / `remove-watch` | `:effect/watch` |
| `render` | `render!`, `clear-cache!` | 模块规则 + schema |
| `interop/js` | `js/...`, `&js-object`, `js-await` | `:effect/js` |
| `interop/host` | `register_import_proc` 注入 proc | descriptor 元数据 |
| `platform` | `register-calcit-platform-api` 能力 | platform API RFC |
| `unknown` | 无法分类的 `:dynamic` 调用 | 需补全 schema |

Effect 边：从 Transform 节点指向 effect 节点；effect 节点可带 `target`（文件路径、DOM、atom 名等）若可静态推导。

**与 State 的边界规则**：

- `assoc state :content x` → **state** 更新（field inout）
- `reset! *counter 1` → **state**（atom persist）+ 可选标 `effect/state-mutate`（若需区分“突变事件”）
- `render! (comp-container ...)` → **effect**（UI 输出），其参数中的 state 仍归 State 分析

第一版建议：**突变写入归 State，对外可观测归 Effect**，避免双重计数。

---

## 4. 与 Koka effect typing 的关系（借鉴而非照搬）

Koka 核心思想：函数类型携带 effect row，例如 `f : int -> console string`。

```koka
fun greet(name: string) : console ()
  println("hello " ++ name)
```

Calcit **不**在第一阶段引入完整 effect handler 语义（见 `02-18-language-theory-evolution-plan.md` 非目标），但借鉴：

| Koka 概念 | Calcit 对应（分阶段） |
|-----------|----------------------|
| effect row `<console, exn>` | `CodeEntry.:schema` 新增可选 `:effects` 列表 |
| pure function `()` | `:effects $ []` 或省略 |
| handled effect | 仅分析期标注，不要求 handler 语法 |
| `perform` / `ctl` | 无；用 builtin / 调用模式识别 |
| row polymorphism | 第二版：`:effects $ [] 'e` 泛型行变量（可选） |

### 4.1 建议的 schema 扩展（Phase B）

在现有 `:: :fn` payload 中增加：

```cirru
:schema $ :: :fn
  {}
    :args $ [] :dynamic
    :return :dynamic
    :effects $ [] :console :render
```

或独立 effect 声明（`defeffect` 已在 snapshot 词法表预留，尚未实现）：

```cirru
defeffect Render
  .mount :fn
  .patch :fn

defn render-once (ui) $
  hint-fn $ {} (:effects $ [] :render)
  render! ui
```

第一版 **不强制** 用户手写；`effects-graph --infer-missing` 可生成建议补丁。

### 4.2 内置 proc 的 effect schema

**首选数据源**：`calcit-core.cirru` 各 builtin 定义的 `:tags` 字段。分析器按 tag 映射到 effect kind，无需维护独立硬编码表。

#### Tag 约定（calcit.core）

| Tag | 含义 | 典型定义 |
|-----|------|----------|
| `:state` | 可变引用 / 本地可变容器 | `defatom`, `atom`, `reset!`, `swap!`, `deref`, `ref?`, `add-watch`, `remove-watch`, `&atom:deref`, `&buf-list:*` |
| `:io` | 宿主 / 运行时交互 | `read-file`, `write-file`, `get-env`, `cpu-time`, `&get-os`, `&get-calcit-*`；`println`/`eprintln`/`echo` 为宿主注入 |
| `:file` | 文件读写（`:io` 子类） | `read-file`, `write-file` |
| `:env` | 环境变量（`:io` 子类） | `get-env` |
| `:log` | 控制台输出（`:io` 子类） | `println`, `eprintln`, `echo`（宿主注入，见 Phase C） |
| `:control` | 控制流中断 | `raise`, `quit!`, `try`；测试宏 `assert`/`assert=`/`assert-detect`（失败时 `raise` + `eprintln`） |
| `:interop` | 宿主 FFI / dylib / 动态求值 | `&call-dylib-edn*`, `eval`, `js-object` |
| `:meta` | 程序自省 / 编译期元数据 | `&get-def-doc`, `&get-def-schema`, `macroexpand*`, `assert-type`, `deftype-slot`, `with-type-slot`, `&data-to-code`, `&extract-code-into-edn` |
| `:async` | 异步标记（经 `hint-fn`） | `hint-fn` |
| `:watch` | atom 监听回调 | `add-watch`, `remove-watch`（与 `:state` 叠加） |
| `:effect` | 显式副作用组合子 | `&doseq` |

映射示例（tag → effects-graph kind）：

- `read-file` (`:io` `:file`) → `[:io/read]`
- `write-file` (`:io` `:file`) → `[:io/write]`
- `raise` (`:control`) → `[:control/raise]`
- `get-env` (`:io` `:env`) → `[:env]`
- `reset!` / `swap!` (`:state`) → `[:state/write]`
- `add-watch` (`:state` `:watch`) → `[:state/watch]`
- `eval` (`:interop`) → `[:interop/eval]`
- `hint-fn` (`:async`) → 分析子函数 `:effects` 行
- `&doseq` (`:effect`) → body 内 effect 边展开
- `println` (`:log` `:io`) → `[:console]`

宿主注入 proc 通过 `RegisteredProcDescriptor.tags`（`HashSet<EdnTag>`，与 core `:tags` 同名）声明；可用 `cr query host-procs [--tag :log]` 查看。

---

## 5. 命令设计

### 5.1 语法

```bash
cr analyze effects-graph [options]
```

与 `call-graph` 对齐的选项：

| 选项 | 说明 | 默认 |
|------|------|------|
| `--root ns/def` | 入口定义 | `:init-fn` |
| `--format tree\|json\|mermaid` | 输出格式 | `tree` |
| `--max-depth N` | 子图展开深度 | 无限制 |
| `--include-core` | 包含 `calcit.core` 节点 | false |
| `--ns-prefix PREFIX` | 只保留匹配 ns 子树 | 无 |
| `--detail summary\|full\|minimal` | Transform 压缩级别 | `summary` |
| `--infer-missing` | 输出类型/effect 补全建议 | false |
| `--show-transform-body` | 在 summary 模式下仍输出骨架 AST | false |

### 5.2 输出格式

## 输出格式（tree）

默认 `--max-depth 1`：先展示入口 **Program Overview**（独立 STE 三棵树），子图以索引列出并标注 `[collapsed]`，按需 `--root ns/def` 或增大 `--max-depth` 展开。

`--max-depth 0`（无限制）时额外输出 **§2 Subgraph Trees**，每个子函数各一棵独立 STE 树。

```text
# Effects Graph
Entry: app.main/main!

## app.main/main!  [program]
├── state
│   ├── import  util.core/log-title
│   └── ...
├── transform (summary)
│   └── sequential: 24 test modules, 3 branches
└── effects
    └── console  println (×N)

    └── app.comp.container/comp-container  [transform]
        ├── state.in   states (:map)
        ├── state.out  states (:map)
        ├── transform  comp-message-box → comp-sessions-modal
        └── effects
            ├── render   respo.core/render!
            └── interop  respo.controller.client/send-to-component!
```

#### json

机器消费；节点类型：`program | transform | state_port | effect`。

#### mermaid（默认，birdview）

专注快速理解程序：**State** 数据结构与类型、**Transform** 关键函数连接、**Effects** 副作用种类。

```mermaid
flowchart LR
  subgraph stateLane["State"]
    s0["states<br/>:map"]
  end
  subgraph transformLane["Transform"]
    t0["main!"]
    t1["comp-container"]
  end
  subgraph effectLane["Effects"]
    e0[[render]]
  end
  t0 -->|call| t1
  t1 -.->|state| s0
  t1 ==>|effect| e0
```

- 蓝 = State（`name<br/>:type`）
- 黄 = Transform（关键 `ns/def` 简名）
- 红 = Effect（按 kind 聚合，隐藏具体 proc 细节）
- `-->` 函数调用 · `-.->` 状态关联 · `==>` 触发副作用

### 5.3 与 `call-graph` 的组合

推荐工作流：

1. `cr analyze call-graph` — 看清**可达定义集合**
2. `cr analyze effects-graph` — 在同一入口上读**语义分解**
3. `cr analyze check-types --infer-missing` + `effects-graph --infer-missing` — 补齐 schema

---

## 6. 分析管线（实现架构）

### 6.1 模块划分

新增 `src/effects_graph.rs`（库模块），CLI 入口挂到 `cr analyze`（`cli_args.rs` / `cr.rs`），与 `call_tree` 并列。

```
effects_graph/
  mod.rs           # 公共类型、入口 analyze_effects_graph()
  extract.rs       # 从 Calcit/Cirru 提取 STE
  classify.rs      # proc/call → effect kind
  state.rs         # 参数、let、atom、assoc 数据流
  transform.rs     # 控制流骨架 + summary 生成
  infer.rs         # 缺 schema 时的补全建议
  format.rs        # tree / json / mermaid
```

**不修改求值语义**；仅在 preprocess 之后的 `PROGRAM_CODE_DATA` + `CompiledDef` 上只读分析（符合 `02-18` 保守试验约束）。

### 6.2 数据来源优先级

| 信息 | 来源 1 | 来源 2 | 来源 3 |
|------|--------|--------|--------|
| 参数/返回类型 | `CodeEntry.:schema` | `hint-fn` | 推断 `weak-types` |
| 文档摘要 | `&get-def-doc` / `entry.doc` | — | — |
| 调用目标 | `call_tree` 同款 `extract_calls` | — | — |
| Effect 种类 | builtin proc 表 | `:effects` schema | 启发式（`js/` 前缀） |
| State 槽 | schema + `assoc`/`get` 模式 | atom 表 | — |

### 6.3 Transform 压缩算法（summary 模式）

1. 对 `defn` 体做**浅层**遍历，深度上限 `D=3`（可配置）
2. 保留：`if/cond/match/foldl/map/filter/let` 节点类型与子节点**类型标签**
3. 替换：字面量 → `_`；长字符串 → `"..."` ；大块 `quote` → `⟨quoted⟩`
4. 生成 `summary`：优先 `doc` 首句，否则模板 `"let×N, if×M, calls K"`

`full` 模式：输出类似 `cr tree show --chunked` 的分片骨架（见 `03-18-query-def-tree-show-chunked-display-plan.md`），但不输出完整叶子。

### 6.4 类型补全（`--infer-missing`）

当某 `ns/def` 缺少 `:schema` 或 `:effects` 时：

1. 用现有 `analyze_code_entry` / type inference 收集 `:args`/`:return` 候选
2. 用 effect 分类器扫描函数体，汇总 effect 集合
3. 输出 unified diff 建议（仅 stdout 或 `--write-suggestions file` 未来扩展）

示例建议块：

```text
## Suggested schema patch: app.comp.container/comp-message-box
:schema $ :: :fn
  {}
    :args $ [] :dynamic
    :return :dynamic
    :effects $ [] :render :console
```

与 `cr edit` 集成留作 Phase C。

---

## 7. 分阶段实施计划

### Phase A — 只读分析 MVP（4~6 周）

**交付**：

- [x] `cr analyze effects-graph` tree + json 输出
- [x] 入口可达分析（复用 `CallTreeAnalyzer` 可达集）
- [x] State：`param` / `return` / `local` / `atom` 基础识别
- [x] Effect：builtin proc 表（core 全覆盖）
- [x] Transform：`summary` 模式 + `calls` 边
- [x] 测试：`calcit/test-effects-graph.cirru`（纯 calcit 小程序，不依赖 respo）

**非目标**：`:effects` schema、`defeffect`、mermaid、自动写回 snapshot。

### Phase B — 类型驱动 + 框架规则（4 周）

**交付**：

- [ ] `CodeEntry.:schema` 支持 `:effects` 列表（解析 + `check-types` 统计）
- [ ] Respo 规则包：`render!`、`d!`、`send-to-component!` 静态识别
- [ ] `--infer-missing` 建议输出
- [ ] `mermaid` 格式
- [ ] 文档：`docs/features/effects-graph.md`

### Phase C — 生态与编辑器集成（后续）

- [x] `RegisteredProcDescriptor.tags`（与 core `:tags` 对齐）
- [ ] `defeffect` 语法落地（可选）
- [ ] `cr analyze effects-graph-diff <git-ref>`（对齐 `program-diff`）
- [ ] Agent 指南：`cr docs agents` 增加 effects-graph 工作流
- [ ] 与 `query def` 联动：`cr query def ns/def --view effects`

---

## 8. 示例：Respo UI 程序片段

入口：`app.comp.container/comp-container`（msg-buffer 类项目）。

预期分解（示意）：

```text
app.comp.container/comp-container
├── state
│   ├── param   states (:map)           # Respo 组件状态
│   ├── param   cursor (:fn)            # d! 回调
│   ├── local   message-box-state
│   └── atom    *abort-control (persist)
├── transform
│   ├── comp-message-box(states, cursor)
│   ├── comp-sessions-modal(...)
│   └── cond done? / streaming? / ...
└── effects
    ├── render      respo.core/render!
    ├── interop     feather.core/comp-i
    ├── console     println (tests only)
    └── watch       (if add-watch present)
```

读者**无需打开** 1500 行 `calcit.cirru` 即可理解：状态在 `states` / `message-box-state`，UI 通过 `render!` 输出，流式中止走 `*abort-control`。

---

## 9. 测试策略

| 层级 | 内容 |
|------|------|
| 单元测试 | `classify_effect`, `extract_state_ports`, `compress_transform` |
| 集成测试 | 对 `calcit/test-effects-graph.cirru` 跑 `cr analyze effects-graph --format json`，快照比对 |
| 回归 | 不改变现有 `call-graph` / `check-types` 行为 |
| 可选 | msg-buffer / respo 手工验收（不纳入 CI 硬依赖） |

---

## 10. 非目标（第一版）

- 不实现 Koka 式 `with/handler` 运行时语义
- 不改变 JS / WASM codegen
- 不做跨进程 / 网络 effect 的自动发现（除非显式 builtin）
- 不保证 whole-program 数据流**完备**（Halting 与动态调用不可判定）
- 不把 `effects-graph` 当作安全沙箱策略

---

## 11. 开放问题

1. **Transform 压缩深度默认值**：`D=3` 是否足够表达 Respo 组件？需用 msg-buffer 实测。
2. **`d!` 语义**：算 state 突变还是 effect？建议 state，但是否要单独 `effect/state-notify`？
3. **宏生成代码**：`quasiquote` 残留是否进入 Transform？建议分析**展开后** IR。
4. **递归节点展开**：同一 `fqn` 多次出现是 inline 子图还是 `seen` 引用（对齐 call-graph）？
5. **`:effects` 与 `:return` 交叉**：`:: :fn {:return :unit}` 且含 `:console` 是否强制标注？建议 warning。
6. **`defeffect` 与 `deftrait` 关系**：effect 是否复用 trait 机制？第一版独立，避免混淆。

---

## 12. 与现有 RFC 的衔接

| 文档 | 关系 |
|------|------|
| `03-05-function-schema-dual-track-rfc.md` | `:schema` 是 State/Transform 签名的主来源；本 RFC 扩展 `:effects` |
| `02-18-language-theory-evolution-plan.md` | 分析层优先、不求值语义变更 |
| `02-17-register-platform-api-rfc.md` | 宿主 proc effect 描述符 |
| `03-16-runtime-boundary-refactor-plan.md` | 长期 state slot 与 ref 显式化可强化 State 分析 |
| `05-12-program-diff-rfc.md` | 未来 `effects-graph-diff` 可对比 STE 结构变化 |

---

## 13. 验收标准（Phase A）

- [ ] `cargo run --bin cr -- calcit/test.cirru analyze effects-graph` 成功退出
- [ ] 输出包含 entry 的 state / transform / effect 三节
- [ ] `read-file` 调用归类为 `io/read`，不落在 transform 摘要正文中
- [ ] `--format json` 可被 `jq` 解析，节点含 `fqn`、`kind`
- [ ] 文档与本 RFC 同步进入 `RFCs/README.md`
