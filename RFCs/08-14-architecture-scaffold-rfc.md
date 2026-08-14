# RFC: 基于 definition graph 的功能架构脚手架

状态：Draft  
日期：2026-08-14  
关联：`08-14-todo-placeholder-rfc.md`、`03-05-function-schema-dual-track-rfc.md`、`05-12-program-diff-rfc.md`、`07-26-safe-structured-editing-rfc.md`、`07-28-persistent-tree-cursor-rfc.md`

## 1. 概要

新增面向 Agent 的功能级架构模式：一次提交一份用 Cirru EDN 描述的 definition graph，CLI 检查已有 definition、schema、namespace 和依赖边，再以单个原子 transaction 创建整组函数或数据定义的脚手架，并输出可以分发给多个 Agent 的实现工作项。

建议命令：

```bash
cr edit scaffold --file docs/architectures/order-submission.cirru --dry-run --format edn
cr edit scaffold --file docs/architectures/order-submission.cirru --expect-revision '<revision>'
```

它与现有编辑原语的关系是：

- `edit def` 仍是提交一个完整 definition 的最小原语；
- `edit transaction` 仍负责一组结构化修改的原子提交；
- `edit scaffold` 是 architecture overlay 到普通 edit operations 的 planner/compiler，不单独发明另一套 Snapshot writer；
- architecture 声明一组期望存在的 definition、接口和 planned edges，不要求同时完成实现；
- scaffold graph 同时包含待创建、项目内已有和 external definition；apply 只生成缺失项，不覆盖已有实现；
- 已有 definition 继续参与 kind/schema 匹配、doc 差异预览、graph 展示和任务上下文；
- 一次 scaffold apply 要么整体写入成功，要么不改变 Snapshot；
- CLI 输出 work items，但不在进程内启动或管理 Agent。

“架构”在本 RFC 中是对 Snapshot 的结构化 desired-state overlay，不是新的运行时语义。生成结果仍是普通 `CodeEntry`；actual call graph、diagnostics、implementation status 和 task batches 都是可重新计算的派生视图。

## 2. 设计原则与范围

Calcit Snapshot 可以视为结构化程序数据库：

| 层 | 内容 | 性质 |
|----|------|------|
| Program Snapshot | namespace、definition、doc/schema/code/examples | 程序事实来源 |
| Architecture overlay | 期望 definition 与 planned edges | 实现期设计契约 |
| Semantic operation | 带 precondition 的结构化编辑 | 可提交变更 |
| Derived view | tree、call graph、work items、drift、diagnostics | 随时重算 |
| Cursor state | 每个 CLI user 的导航与 clipboard | 本地临时状态 |

第一版优先打通以下闭环：

```text
architecture
  -> normalize and reconcile
  -> atomic scaffold
  -> work items
  -> Agents implement definitions
  -> parent serially applies changes
  -> type/test/call-graph verification
```

第一版不尝试：

- 自动生成完整业务逻辑；
- 根据自然语言猜测 schema；
- 自动解决不兼容的已有 definition；
- 在同一 Snapshot 上提供多个写进程的并发安全；
- 在 CLI 内启动、租约管理或监控 Agent；
- 用 cursor activity 充当任务锁或写入正确性机制；
- 实现 definition-level semantic patch merge；
- 以 planned graph 宣称实际实现一定遵守所有边。

多 Agent 的第一版并行发生在“实现内容的产出”，最终 Snapshot 修改仍由 parent/coordinator 串行提交。这样先复用现有 transaction/revision 能力，避免为了调度器、锁和 merge 协议阻塞 architecture 流程。

## 3. 输入模型：平面规范，树形展示

### 3.1 为什么 canonical model 是平面图

调用关系适合按入口向下展示，但真实程序存在共享依赖、递归和环。若把嵌套 tree 作为权威存储，就必须处理重复完整声明、`:ref` 合并和树中环编码。

第一版因此直接采用平面 canonical model：

- `:definitions` 是以 FQN Symbol 为 key 的 map；
- `:edges` 是 typed edge 的 set；
- `:roots` 是入口 definition set；
- map/set 的顺序不参与语义；
- human renderer 从 graph 投影出 call tree，对重复节点标 `[seen]`，对环标 `[circular]`；
- 嵌套 tree 输入可以作为后续语法糖增加，但不得形成第二种内部模型。

definition identity 统一使用 Cirru EDN Symbol，例如 `'app.order/submit-order!`，不在输入中把 FQN 表达成普通 String，再由 parser 隐式转换。

### 3.2 Cirru EDN v1

```cirru
{}
  :schema-version 1
  :feature 'order-submission
  :doc "|Validate, persist, and notify for a submitted order."
  :roots $ #{} 'app.order/submit-order!
  :definitions $ {}
    'app.order/submit-order! $ {}
      :mode :ensure
      :kind :fn
      :doc "|Validate one order and persist it when accepted."
      :params $ [] 'order 'context
      :schema $ :: :fn
        {}
          :args $ [] 'Order 'RequestContext
          :return 'OrderResult
    'app.order/validate-order $ {}
      :mode :ensure
      :kind :fn
      :doc "|Return normalized order data or validation errors."
      :params $ [] 'order
      :schema $ :: :fn
        {}
          :args $ [] 'Order
          :return $ :: :tuple :ok 'Order
    'app.order/persist-order! $ {}
      :mode :ensure
      :kind :fn
      :doc "|Persist an accepted order."
      :params $ [] 'order 'context
      :schema $ :: :fn
        {}
          :args $ [] 'Order 'RequestContext
          :return $ :: :tuple :ok 'OrderId
    'app.order/order-total $ {}
      :mode :ensure
      :kind :fn
      :doc "|Calculate the total payable amount."
      :params $ [] 'order
      :schema $ :: :fn
        {}
          :args $ [] 'Order
          :return :number
    'app.order/OrderResult $ {}
      :mode :ensure
      :kind :data
      :doc "|Public result returned by order submission."
      :schema 'OrderResult
      :code $ quote
        defenum OrderResult
          :accepted OrderId
          :rejected ValidationError
    'app.db/save-record! $ {}
      :mode :external
      :kind :fn
      :schema $ :: :fn
        {}
          :args $ [] 'Order 'RequestContext
          :return $ :: :tuple :ok 'OrderId
  :edges $ #{}
    :: :call 'app.order/submit-order! 'app.order/validate-order
    :: :call 'app.order/submit-order! 'app.order/persist-order!
    :: :type 'app.order/submit-order! 'app.order/OrderResult
    :: :call 'app.order/validate-order 'app.order/order-total
    :: :call 'app.order/persist-order! 'app.db/save-record!
```

`:schema` 直接使用当前 `CodeEntry.:schema` 的 canonical EDN，不加 `quote`。只有 `:code` 保存 Cirru AST，因此使用 `quote`。

### 3.3 顶层字段

| 字段 | 必需 | 含义 |
|------|------|------|
| `:schema-version` | 是 | architecture 输入版本，第一版为 `1` |
| `:feature` | 是 | 稳定的 feature Symbol，用于报告与 plan identity |
| `:doc` | 否 | 功能目标和边界说明 |
| `:roots` | 是 | 一个或多个入口 definition Symbol |
| `:definitions` | 是 | FQN Symbol 到 definition declaration 的 map |
| `:edges` | 否 | typed edge set；缺省为空 set |

planner 对 canonicalized architecture 计算 `plan-id` 内容 hash。`plan-id` 用于报告、work item 和 drift 对应，不写入运行时语义。

### 3.4 definition declaration

| 字段 | `:fn` | `:data` | 说明 |
|------|-------|---------|------|
| `:mode` | 必需 | 必需 | `:ensure` 表示项目中应存在；`:external` 表示只验证、不创建 |
| `:kind` | 必需 | 必需 | 第一版为 `:fn` 或 `:data` |
| `:doc` | ensure 必需 | ensure 必需 | 创建时写入 `CodeEntry.doc`；已有节点只比较、不覆盖 |
| `:schema` | 必需 | 必需 | 当前 canonical annotation；已有节点作为兼容性断言 |
| `:params` | ensure fn 必需 | 禁止 | Cirru EDN Symbol list；数量必须与 schema 对应 |
| `:code` | 可选 | 第一版必需 | function 缺省时生成 TODO stub；data 暂不猜测语法形状 |
| `:tags` | 可选 | 可选 | 创建时并入 `CodeEntry.tags` |
| `:examples` | 可选 | 可选 | 创建时写入 `CodeEntry.examples`，用于提供行为契约 |

`:mode :ensure` 是“存在或复用”，不是“强制覆盖成声明内容”。当同名 definition 已存在时，planner 同时保留 existing/planned 视图并执行兼容性检查。

### 3.5 typed edges

第一版 edge 是匿名 enum 实例：

```cirru
:: :call 'caller/a 'callee/b
:: :type 'consumer/a 'types/T
```

- `:call` 是 edge tag，表示计划中的直接函数调用；
- `:type` 是 edge tag，表示 schema 或数据层依赖；
- edge 两端都必须出现在 `:definitions`；
- target 可以是 `:ensure` 或 `:external`；
- 第一版只实现 `:call` 与 `:type`，未来再兼容增加其他 edge kind。

typed edge 是 architecture 意图，不直接写入 `CodeEntry`，也不在 TODO body 中伪造不可执行调用。actual call/type dependency 由实现后的程序重新分析。

call graph 不是 task dependency graph。atomic scaffold 已经先建立全部名称、schema 和 stub，因此 caller 通常可以只依赖接口并行实现。SCC、叶子优先和拓扑层只作为未来调度建议，不成为第一版 work item 的硬约束。

### 3.6 function stub

无 `:code` 的 ensure function 生成普通 definition：

```cirru
defn validate-order (order)
  todo! "|implement app.order/validate-order; planned calls: app.order/order-total"
```

要求：

- 名称和参数来自声明，不从 doc/schema 猜测；
- `:params` 每一项必须是 `Edn::Symbol`，不接受 String/Tag 隐式转换；
- body 使用 compiler-known `todo!`，internal Never 可满足声明返回类型；
- 静态检查产生带 definition/path 的 `W_TODO`，运行时抵达则明确失败；
- TODO 文本仅提供人类上下文，机器关系仍以 typed edges 为准；
- stub 带 `:scaffold` tag；完成实现后由编辑命令或检查器移除；
- schema、doc、examples、tags 和 code 在同一次 Snapshot 写入中提交；
- scaffold apply 接受本次新 stub 的 expected `W_TODO`，完成门禁仍在 TODO 尚存时失败。

具体行为见 `08-14-todo-placeholder-rfc.md`。

## 4. CLI 与 reconciliation

### 4.1 主命令

```bash
cr edit scaffold --file <architecture.cirru> [options]
```

第一版参数：

```text
--code <cirru-edn>              小型单值输入
--file <path>                   推荐的可审阅输入文件
--dry-run                       只规范化、检查和报告
--expect-revision <hash>        Snapshot 级 stale-write 防护
--format human|edn|json         默认 human；EDN 是规范机器格式
```

第一版固定采用“compatible existing 复用、hard conflict 拒绝、缺失 function 生成 TODO stub”，不为唯一策略增加开关。scaffold 也不提供 `--overwrite`；批量覆盖已有 definition 属于 reconciliation/semantic patch，而不是脚手架生成。

### 4.2 执行阶段

1. 读取并验证 Cirru EDN；
2. canonicalize definitions、roots、typed edges，计算 `plan-id`；
3. 读取一次 Snapshot，计算 snapshot revision；
4. 验证 ensure namespace 可编辑，external 可从 project/dependency/core 解析；
5. 将节点分类为 `create`、`reuse-pending`、`reuse-complete` 或 `external`；
6. 比较 existing/planned kind、schema、doc，并产生 diagnostics；
7. 把全部 create 节点编译成现有 edit/transaction operation；
8. 在内存 Snapshot 应用、解析并预处理受影响 definitions；
9. 生成 normalized plan、reconciliation、operations、work items 和 proposed revision；
10. dry-run 到此结束；apply 复用 transaction staged write 与 atomic rename；
11. 成功后只维护当前 CLI user 的 cursor，并建议首个实现 target。

parse、输入自冲突、不可编辑 namespace、无法解析的 external、缺失 edge endpoint 或具体类型冲突都在写入前结束。warning 不阻止 apply，但机器结果必须结构化返回。

### 4.3 已有 definition

已有 definition 是 graph 的一等节点。planner 并列保存 current fact 与 architecture expectation：

- tree 中保留节点及其 planned edges，状态标为 `reuse-pending` 或 `reuse-complete`；
- 记录 `origin`：`project`、`dependency` 或 `core`；
- `existing` 保存当前 doc/schema/kind；
- `planned` 保存 architecture doc/schema/kind；
- apply 不修改已有 code/doc/schema/examples/tags；
- compatible existing 节点已经提供接口；若仍带 `:scaffold` tag 或 scaffold TODO，则标为 `reuse-pending` 并继续生成稳定 work item，否则标为 `reuse-complete`；
- conflict 默认拒绝整次 apply。

`reuse-pending` 使 architecture workflow 可以中断后恢复：apply 后重新 dry-run 不会因为 definition 已经存在而丢失待实现任务。完成 definition 必须替换 TODO body 并移除 `:scaffold` tag；planner 随后才不再输出该 work item。

第一版兼容规则保持保守：

| 情况 | 默认行为 | 级别 |
|------|----------|------|
| canonical schema 与 kind 完全相同，且已完成 | `reuse-complete` | info |
| canonical schema 与 kind 完全相同，仍有 scaffold/TODO | `reuse-pending`，继续输出 work item | info |
| existing Dynamic、planned concrete | 复用，不自动收窄 | warning |
| planned Dynamic、existing concrete | 复用已有强约束 | warning |
| 参数数量或 kind 不兼容 | 不写入 | conflict |
| 两边均 concrete 但 schema 不同 | 不写入 | conflict |
| doc 不同 | 保留 existing，展示差异 | warning |
| external 在 project 中存在 | 解析为 existing external | info |
| ensure 实际来自 dependency/core | 提示改为 external | conflict |

不要在 scaffold 中发明子类型系统；未来只复用统一 type unification。

### 4.4 机器结果

Cirru EDN stdout 是单个 map：

```cirru
{}
  :schema-version 1
  :ok true
  :command :edit-scaffold
  :feature 'order-submission
  :plan-id |md5:plan
  :dry-run true
  :snapshot-revision |md5:current
  :proposed-revision |md5:proposed
  :normalized-plan $ {}
    :roots $ #{} 'app.order/submit-order!
    :definitions $ {}
    :edges $ #{}
  :reconciliation $ {}
    'app.order/validate-order $ {}
      :status :reuse-complete
      :origin :project
      :existing $ {}
        :doc "|Existing validation entry."
        :schema :dynamic
      :planned $ {}
        :doc "|Return normalized order data or validation errors."
        :schema $ :: :fn
          {}
            :args $ [] 'Order
            :return $ :: :tuple :ok 'Order
      :diff $ #{} :doc :schema
  :operations $ []
  :work-items $ []
  :diagnostics $ []
```

human renderer 从同一 typed result 展示 call tree、create/reuse-pending/reuse-complete/external 统计和字段差异。EDN 是规范机器格式；JSON 只作为现有 `JSON.parse`、`jq`、LSP/MCP adapter 和历史脚本的 compatibility projection。两种机器 renderer 的 stdout 都只能包含一个 value，日志与 command echo 走 stderr。

## 5. 与 actual call graph 的关系

architecture graph 是意图，actual call graph 是从已有代码提取的事实。二者共享 definition identity、typed edge 基本形状和 tree renderer，但不共享语义类型。

- scaffold 前，architecture graph 可以描述尚不存在的程序；
- scaffold 后，TODO stub 的 actual call edges 可以为空并标记 pending；
- 实现后，可比较 planned `:call` edge 与 actual direct call edge；
- `:type` edge 应与 schema/type dependency 分析比较；
- drift 是派生报告，不自动修改 architecture 或代码。

实现上抽出无状态 graph formatter/statistics；scaffold parser 不依赖全局 `PROGRAM_CODE_DATA`。

## 6. 多 Agent 实现流程

### 6.1 第一版协作模型

CLI 只负责产生结构化 work items。parent agent 或外部 orchestrator 负责分发：

1. scaffold apply 原子创建所有缺失接口和 TODO stub；
2. 每个 create 或 reuse-pending function 形成一个默认 work item；
3. parent 把互不重叠的 target 分配给 subagent；
4. subagent 基于稳定 schema/doc/edges/examples 产出 definition 实现；
5. parent 使用现有 `edit def` 或 `edit transaction` 串行写回 Snapshot；
6. 每次写回使用最新 Snapshot revision，并运行按影响范围选择的检查；
7. 全部 work item 完成后检查残留 `W_TODO`、类型、测试和 graph drift。

这种模型允许并行完成代码设计和实现，但只保留一个 Snapshot writer。它不需要第一版实现 daemon、lease、共享锁或三方 AST merge。

### 6.2 work item

默认 work item 至少包含：

```cirru
{}
  :id 'order-submission/implement-validate-order
  :plan-id |md5:plan
  :target 'app.order/validate-order
  :base-snapshot-revision |md5:scaffolded
  :write-set $ #{} 'app.order/validate-order
  :doc "|Return normalized order data or validation errors."
  :schema $ :: :fn
    {}
      :args $ [] 'Order
      :return $ :: :tuple :ok 'Order
  :planned-edges $ #{}
    :: :call 'app.order/validate-order 'app.order/order-total
  :examples $ []
```

规则：

- work item ID 与 target 稳定，不包含运行时 Agent 名称；
- `write-set` 表示默认允许修改的 definition；parent 扩大范围时必须显式处理重叠；
- cursor user 是执行 Agent 的导航身份，由 parent 分配，不属于 architecture；
- call graph 邻接不代表 write-set 重叠；第一版不计算强制 batch；
- reuse-complete/external 节点不产生实现 work item，但会进入相邻 target 的上下文；
- reuse-pending 节点在重复 dry-run/apply 后继续产生相同 ID 的 work item，直到 TODO 与 scaffold tag 被清理；
- namespace import/config/test 等共享修改由 parent 集中处理，或在未来作为独立 typed operation 建模。

### 6.3 后续 semantic patch

Snapshot-level `--expect-revision` 安全但保守：两个 Agent 修改不同 definition，也会因全局 revision 变化而需要重新读取。流程走通后，可增加 definition-level semantic patch：

```cirru
{}
  :target 'app.order/validate-order
  :expect-definition-revision |md5:base-definition
  :operation :replace-code
  :code $ quote ...
```

新 definition 使用 `:expect :absent`。parent/coordinator 在最新 Snapshot 上串行 apply：互不相交的 patch 可自动接受，同一 definition 的重叠修改精确拒绝。

这是后续优化，不阻塞第一版。即使加入 semantic patch，物理 Snapshot 写入仍保持 single writer；并发发生在 patch 产出，而不是最终 rename。

## 7. Cursor user 的边界

cursor user 用于区分 CLI 导航状态，不是 work item owner、权限身份或并发锁。parent 可以为 subagent 设置：

```bash
cr --cursor-user agent-a cursor set app.order/validate-order --path @0
CALCIT_CURSOR_USER=agent-b cr cursor show
cr cursor show # fallback: default
```

解析优先级仍为 `--cursor-user` > `CALCIT_CURSOR_USER` > `default`。

第一版 architecture/scaffold 不依赖完整 cursor user 实现。scaffold 成功后只维护发起命令的当前 cursor；其他 user 在下次访问自己的 cursor 时按 revision/fingerprint 懒惰校验，不要求一次 source mutation 重写所有 user 状态。

后续若实现多 user 持久化，推荐每 user 一个文件，而不是所有进程改同一个 `:users` map：

```text
.calcit/cursors/default.cirru
.calcit/cursors/agent-a.cirru
.calcit/cursors/agent-b.cirru
```

这样 navigation/clipboard 不会因共享 sidecar 的 read-modify-rename 发生 lost update。`cursor users` 可通过目录发现；每个文件仍使用 atomic rename。旧 v1-v4 sidecar 可在显式迁移时把 active entry 写入 `default`，其他合法 named entry 写入同名 user 文件。

activity/overlap warning、lease 和 heartbeat 暂不实现。任务重叠优先由 parent 比较 work item `write-set`，写入正确性继续依赖 revision/precondition。

## 8. 原子性与失败恢复

scaffold 复用 transaction 原则：

- 输入解析、reconciliation 和全部 conflict 检查先于写入；
- 所有 create operation 在一个内存 Snapshot 中应用；
- staging 后再次确认 `--expect-revision`；
- Snapshot 通过同目录 staged file 和 atomic rename 提交；
- Snapshot 成功后才维护当前 user cursor；
- cursor 保存失败报告“源码已提交、cursor 未更新”，不得伪装为整体回滚；
- 相同 architecture 再次 apply 必须保持 Snapshot 幂等，ensure 节点成为 reuse-pending 或 reuse-complete；
- architecture 输入、work items 和 cursor activity 不写入运行时 Snapshot。

第一版不尝试跨 Snapshot/cursor 文件做单一原子提交，也不支持同 Snapshot 多 writer。

## 9. Architecture 文件生命周期

需要纳入版本控制、作为可审阅设计契约的 architecture 文件，推荐放入：

```text
docs/architectures/<feature>.cirru
```

CLI 仍接受任意 `--file`。目录只是约定，不由第一版自动创建、归档或删除。`.calcit/` 只保留 cursor 等本地临时状态，不作为需要落库的 architecture plan 位置。

- implementation 期间可以纳入版本控制，作为可审阅设计契约；
- normalized content hash 形成 `plan-id`；
- status、diagnostics、work items、actual graph 和 activity 均不回写 architecture 文件；
- 功能完成后，项目可以归档或删除一次性 plan；
- 若长期保留，它才承担 planned-vs-actual drift contract。

第一版只保证 plan 可重复 dry-run/apply，不实现 plan registry 或生命周期管理。

## 10. 分阶段实现

### Phase 0：规范与兼容基础

- RFC、canonical schema 和可解析 Cirru EDN fixture；
- 修正所有示例为当前 unquoted `:: :fn` schema；
- cursor v4 无损保留 named entries，新建默认名称使用 `default`；
- 抽出可复用 Snapshot revision、staged commit 和 typed result renderer。

### Phase 1：dry-run planner

- 平面 definitions/typed edges parser 与 canonicalization；
- Symbol FQN、root/edge 完整性和 plan-id；
- existing/external/kind/schema/doc reconciliation，以及可恢复的 reuse-pending 状态；
- human/EDN graph、diagnostics、operations 和一-definition-per-item work items；
- JSON compatibility projection；
- 不写 Snapshot 的单元与 CLI 协议测试。

当前实现进度（2026-08-14）：`cr edit scaffold` 已支持 `--file`、`--code` 或 stdin 输入，以及 `human`、`edn`、`json` 输出。它验证 Symbol FQN/params、anonymous-enum edge、schema、roots/edge endpoint，计算 `plan-id`，对当前 Snapshot 做 create/reuse/external reconciliation，并输出 create operation 预览和 work items。`--dry-run` 保持只读；不带该 flag 时，会在同目录 staged file 中创建全部缺失的 `:ensure` definition，写入 doc/schema、`:scaffold` tag 和 function `todo!` stub，复核 revision 后以 atomic rename 提交。已有 definition 从不被覆盖；成功 apply 后会触发既有 cursor 后置校验。dependency/core external lookup、definition-level patch 和 per-user cursor 仍留在后续阶段。

### Phase 2：atomic scaffold apply（基础版本已实现）

- `todo!`、`W_TODO`、function/data `CodeEntry` operation 生成；
- 复用 staged Snapshot、revision precondition 和 atomic write；
- apply 后调用现有 cursor 后置校验；
- 已覆盖幂等 apply、部分 conflict 零写入和 function stub 原子创建；
- internal `Never` 的完整 branch/generic inference 和 cursor partial-success reporting 仍待后续统一控制流分析。

### Phase 3：分布式实现闭环

- parent/subagent 使用 work item 的推荐流程和文档；
- definition 实现完成时移除 scaffold tag/TODO；
- 残留 TODO、类型、examples/tests 和 actual graph 检查；
- Respo 等真实项目 Snapshot 副本回归；
- 验证多个 subagent 并行产出、parent 串行应用的完整 feature。

### Phase 4：按实际需要演进

- definition-level semantic patch/precondition；
- `ensure-import` 等可合并 namespace operation；
- per-user cursor 文件与 lazy revalidation；
- planned/actual graph drift；
- SCC/batch 建议、更丰富 edge kind；
- 只有基准或真实冲突证明必要时，才评估 writer coordinator/daemon。

## 11. 验收标准

- 一个 Cirru EDN plan 可声明多个 root、共享依赖、external、递归和 typed edges；
- definition ID 与 params 使用 Symbol，schema 使用当前 canonical unquoted EDN；
- map/set 顺序不影响 normalized plan-id；
- dry-run 不修改 Snapshot/cursor，EDN stdout 是单个可解析 value；
- 已有 definition 在 tree/reconciliation 中可见，包含 origin、existing/planned 和字段差异；
- compatible existing definition 被复用且不覆盖，具体 kind/schema 冲突使整个 apply 零写入；
- apply 一次创建全部缺失 definition，function stub 使用 `todo!` 并产生精确 `W_TODO`；
- 再次 apply 同一输入保持 Snapshot 幂等，未完成的 reuse-pending work item 仍可恢复；
- 每个待实现 function 产生稳定 work item，含 target、write-set、schema、doc 和 planned edges；
- work item 不绑定 cursor user，也不把 call adjacency 当成写入冲突；
- parent 能把多个 work item 分发给 subagent，并通过现有 edit/transaction 串行写回；
- 完成后可检查残留 TODO、类型、测试和 planned/actual call edge；
- scaffold 成功后的 cursor revision/fingerprint 可验证，sidecar 失败报告 partial success；
- `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test`、`yarn compile`、`yarn check-agent-interface` 和 `yarn check-all` 通过。

## 12. 待定问题

1. `:params` 是否允许 destructuring？第一版只接受 Symbol list，复杂参数由显式 `:code` 提供。
2. data definition 的 schema/code 一致性如何统一校验？应复用 struct/enum/trait preprocess，不做文本 token 比较。
3. `:examples` 第一版是否直接写入 `CodeEntry.examples`，还是先只进入 work item？推荐能够通过现有 examples 校验时直接写入。
4. semantic patch 的 typed operation 形状留到 Phase 4，第一版继续复用现有 transaction argument list。
5. architecture 长期保留时，哪些 typed edge 算 hard drift contract？第一版只报告，不阻止普通 build。
