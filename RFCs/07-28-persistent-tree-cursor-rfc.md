# RFC: 可持续迁移的树形 Cursor

状态：Draft
日期：2026-07-28
关联：`07-06-semantic-tree-navigation-rfc.md`、`07-26-safe-structured-editing-rfc.md`、`03-18-query-def-tree-show-chunked-display-plan.md`

## 1. 目标

Calcit 源码以 EDN tree 保存，数字 path 只是某个 snapshot revision 下的瞬时坐标。复杂表达式需要连续执行 show、insert、wrap、replace、delete 等操作时，Agent 即使第一次选对节点，也可能因为前方兄弟节点增删而继续使用已经漂移的 path。

引入项目本地 `.calcit/cursor.cirru` 与 `calcit cursor`，保存当前选择的 namespace、definition 和 tree path。Cursor 不是新的源码身份，也不写进 snapshot；它是 CLI 在多次调用之间维护的结构化选择状态。`.calcit/` 同时作为 error、snippets 与后续模块链接等小型项目本地工件的统一目录，避免继续增加顶层隐藏文件。

核心要求：一旦 cursor 已存在，任何作用于同一 definition 的 tree mutation 都必须尝试迁移 cursor。能够确定新位置时更新坐标并提示；不能确定时明确标为 stale 或移动到可证明安全的父节点，不允许静默指向另一个节点。

## 2. 文件格式

当前只有一个 active cursor；named marks 只是有限位置书签，不是并发 cursor。schema v2 在 v1 的 selection 上增加历史、显式栈和结构化 clipboard；schema v3 移除每个位置中重复保存的完整 subtree preview；schema v4 增加单一 region anchor、最多 16 个 marks 和不保存结果集的 last query。读取器接受 v1-v3，旧 `.calcit-cursor.cirru` 在首次读取时一次性移动到新目录：

```cirru
{}
  :schema-version 4
  :active :default
  :cursors $ {}
    :default $ {}
      :snapshot |calcit.cirru
      :target |app.main/render!
      :section :code
      :path $ [] 3 2 1
      :definition-revision |md5:...
      :fingerprint |md5:...
  :history $ []
    {}
      :snapshot |calcit.cirru
      :target |app.main/render!
      :section :code
      :path $ [] 3 2
      :definition-revision |md5:...
      :fingerprint |md5:...
  :stack $ []
  :anchor $ {}
    :snapshot |calcit.cirru
    :target |app.main/render!
    :section :code
    :path $ [] 3 2 0
    :definition-revision |md5:...
    :fingerprint |md5:...
  :marks $ {}
    :render-start $ {}
      :snapshot |calcit.cirru
      :target |app.main/render!
      :section :code
      :path $ [] 3 2
      :definition-revision |md5:...
      :fingerprint |md5:...
  :last-query $ {}
    :command |search
    :pattern |render-item
    :filter |app.main/render!
    :exact true
    :regex false
    :max-depth 0
    :start-path nil
    :entry nil
    :pattern-is-json false
    :selected-index 2
    :snapshot-revision |md5:...
  :clipboard $ {}
    :mode |cut
    :source-target |app.main/render!
    :source-path $ [] 3 2 1
    :fingerprint |md5:...
    :tree $ quote
      map items $ fn (item)
        render-item item
```

- `:path` 是快速坐标，不是跨 revision 的身份；
- `:fingerprint` 校验 path 处仍是预期 subtree；
- `:definition-revision` 用于发现外部修改；
- `:section` 第一版只接受 `:code`，为 schema、example 与结构化 docs 预留空间。
- `:history` 由 `set` 和导航命令维护，最多保存 32 个位置，供 `back` 使用；
- `:stack` 只由 `push` / `pop` 控制，最多保存 16 个位置，不与普通导航历史混用；
- `:anchor` 与 active cursor 必须是同一 parent 下的 sibling 才形成 region；不保存复制的 subtree；
- `:marks` 最多 16 个，每个只保存位置、revision 和 fingerprint；
- `:last-query` 只保存查询参数、当前 index 和 revision，`query next/prev` 每次重新解析并计算结果，不持久化结果列表；
- `:clipboard` 保存真实 Cirru subtree，不保存经过 formatter 的文本；`copy` 和 `cut` 都可写入，`paste` 后仍保留以支持重复粘贴。

cursor 文件位于 snapshot 同目录的 `.calcit/`，使用目标文件所在目录的临时文件加 rename 更新。`.calcit/` 应作为本地状态加入 `.gitignore`，不参与模块发布或项目语义。首次 `cursor set` 若未检测到相应 ignore 规则，CLI 在 stderr 给出提示但不自动修改项目文件。文件硬上限为 64 KiB；history、stack、marks 都有固定条目上限，只有 clipboard 允许保存源码 subtree，超过上限时先拒绝写入而不截断数据。

## 3. CLI 契约

```bash
calcit cursor set app.main/render! --path @3.2.1
calcit cursor show
calcit cursor parent
calcit cursor child                 # first child
calcit cursor child 2
calcit cursor child --last          # last child
calcit cursor next --count 3
calcit cursor prev --count 2
calcit cursor forward --count 8
calcit cursor backward --count 5
calcit cursor back --count 4
calcit cursor push
calcit cursor pop
calcit cursor anchor
calcit cursor region
calcit cursor clear-anchor
calcit cursor mark render-start
calcit cursor goto render-start
calcit cursor marks
calcit cursor rm-mark render-start
calcit cursor apply swap-next
calcit cursor apply wrap --code 'quote $ when visible? self'
calcit cursor slurp-next
calcit cursor slurp-prev
calcit cursor barf-last
calcit cursor barf-first
calcit cursor duplicate --at after
calcit cursor copy
calcit cursor cut
calcit cursor paste --at after
calcit cursor clipboard
calcit cursor clear-clipboard
calcit cursor clear
```

definition-oriented query/tree/edit 命令的 target 与 tree path 都可用 `@cursor` 引用 active cursor：

```bash
calcit query context @cursor --format json
calcit query type-at @cursor --path @cursor --format json
calcit tree show @cursor --path @cursor
calcit tree replace @cursor --path @cursor --code 'quote $ render-list items'
calcit tree wrap @cursor --path @cursor --code 'quote $ when visible? self'
calcit edit split-def @cursor --path @cursor --name render-items
```

对连续编辑中最常用的操作，`cursor apply <operation>` 进一步省略重复的 target 和 path，但内部仍构造并调用既有 tree 命令，不另外实现 mutation 语义。支持 `delete`、`swap-next`、`swap-prev`、`unwrap`、`raise`、`replace`、`wrap`、`insert-before`、`insert-after`、`insert-child` 与 `append-child`。其中 `unwrap` 的语义是把选中 list 的所有 child 展开到 parent，不承诺与包含额外语法节点的 wrap 模板严格互逆。

`cursor slurp-next/slurp-prev` 与 `cursor barf-last/barf-first` 构成双向 Paredit 风格复合命令：slurp 把相邻 sibling 移入选中 list 对应一端，barf 把首/末 child 移到 list 外对应一侧。能够安全表达为通用 node move 的操作复用 `edit mv`；跨 parent 删除会使 destination 暂时失效的 `barf-first` 使用一次内存树变换。`cursor duplicate --at before|after` 直接复制当前表达式并选中新副本，不污染 clipboard。复合 Snapshot/sidecar 改动都先 stage 两个文件，再按 Snapshot→cursor 顺序提交并报告 partial success。

对双 target 命令，`@cursor` 表示 source；destination 必须显式给出。显式 target/path 与 cursor 的 `namespace/definition` 不一致时拒绝执行。transaction operation 文件必须保持自包含，使用 concrete target/path，不解析依赖外部可变状态的 cursor alias。后续可增加 `@cursor:<name>`，当前不承诺该语法。

`cursor show` 默认调用 Cirru Parser 0.2.15 的 `focus_cirru_preview_with_options`，通过 `CirruFocusOptions` 在 definition 级展示副本中折叠无关分支、保留 definition 的 head/name/参数，并直接使用 `CURSOR` marker。Calcit 不再遍历重写 `'FOCUSED` 或手工拼接 definition header；该依赖使用精确版本约束，防止全局安装忽略 lockfile 时出现未经验证的展示语义漂移。目标表达式只在展示副本中渲染为：

```cirru.no-check
CURSOR
  map items $ fn (item)
    render-item item
```

`CURSOR` 只存在于 presentation AST。真实 path、fingerprint、JSON tree 和任何 mutation 都基于未包裹的源码树。`cursor show --view node|focus|full` 可切换只看节点、结构聚焦或完整 definition；机器输出分别提供真实 `tree` 与带展示标记的 `preview_tree`，不要求调用方从展示代码反推 path。

编辑后的 cursor 回显由顶层参数控制：

```bash
calcit --cursor-after none calcit.cirru tree replace app.main/render! --path @cursor --code 'quote nil'
calcit --cursor-after summary calcit.cirru edit cp app.main/render! --from @3 --path @cursor
calcit --cursor-after focus calcit.cirru tree wrap app.main/render! --path @cursor --code 'quote $ when ok? self'
```

`summary` 是默认值，只向 stderr 输出 target、更新后 path 与迁移原因；`focus` 额外输出结构聚焦预览；`none` 关闭自动回显。三种模式都不改变 cursor 的实际维护。

导航命令按当前 snapshot 的真实树验证边界：`child` 省略 index 时进入首子节点，`child --last` 按当前 child count 进入末子节点；`next` / `prev` 的 `--count N` 一次跨越 N 个 sibling；`forward` / `backward` 按 definition 的深度优先结构顺序跨 list 边界移动；`back --count N` 一次回退 N 条普通导航历史。所有多步移动只写一条 history。`--count 0`、越界和同时传 child index 与 `--last` 都拒绝执行，且不改变 cursor。顶层 `--cursor-after focus` 用于 set、search 选中和导航时，会紧接成功提示展示新的 focus，不要求 Agent 再调用一次 `cursor show`。

`back` 只回退 cursor 位置，不撤销 Snapshot mutation。源码恢复仍应使用版本控制或显式的反向结构编辑，避免把导航历史误当成 source undo。

搜索结果可直接成为 cursor，不需要 Agent 从展示文本复制 target/path：

```bash
calcit query search render-item --filter app.main/render! --exact --set-cursor 0
calcit query search-expr 'map items' --filter app.main/render! --set-cursor 1
calcit query search state --start-path @cursor --set-cursor 0
calcit query search-expr 'div $ {}' --start-path @cursor
calcit query next
calcit query prev
```

`query search` 与 `query search-expr` 在 human 输出为每个 match 显示稳定的全局 `[#N]`，JSON match 增加 `cursor_index`。`--filter @cursor` 限定当前 definition；`--start-path @cursor` 同时推导 definition filter 并把搜索根限制到当前 subtree，冲突 filter 直接报错。显式 `--set-cursor N` 采用同一排序后的结果，在返回查询结果前更新 sidecar；成功提示写 stderr，因此 `--format json` 的 stdout 仍是单个 JSON。越界或 dependency-only match 无法映射到当前可编辑 snapshot 时拒绝设置，并提示用 `--filter <project-namespace>` 缩小范围。`query next/prev` 从 last query 计算相邻 index、重新执行搜索并只回显新 cursor；若 snapshot revision 已变化则拒绝复用旧 index，要求重新执行原搜索选中结果，避免结果重排后跳错节点。

## 3.1 高频开发场景覆盖

- **定位后连续小改**：search `--set-cursor` 后用 `cursor apply`，无需复制容易漂移的数字路径。
- **组件/分支样板扩展**：`cursor duplicate` 后继续 replace/wrap；clipboard 保留给跨位置搬运。
- **调整调用或属性层级**：双向 slurp/barf、swap、wrap/unwrap/raise 覆盖 Lisp 编辑器最常见的结构操作。
- **在大表达式内二次查找**：`search/search-expr --start-path @cursor` 只遍历选中 subtree，并可把结果再次设为 cursor。
- **连续浏览搜索命中**：首次 `--set-cursor` 后使用 `query next/prev`，不再重复长查询参数或保存完整结果集。
- **连续 sibling 范围确认**：在起点 `cursor anchor`，移动到终点后用 `cursor region` 验证结构化范围；region 第一版不隐式改变单表达式 copy/cut 语义。
- **跨 definition 往返**：用最多 16 个 `cursor mark/goto` 保存高频位置，普通短期返回仍用 push/pop。
- **编辑后语义确认**：`query type-at @cursor --path @cursor`、`query context @cursor` 复用当前 definition/selection。
- **抽取与跨位置重构**：`edit split-def @cursor --path @cursor`，或 push/cut/search/pop/paste 组合保留起点和结构化代码。
- **definition 元数据维护**：schema、examples、doc、tags 等 edit target 接受 `@cursor`，definition rename/move 后 cursor 跟随。

workspace/module 解析、全项目分析、namespace 批量更新与可复现 transaction 输入不依赖 cursor；这些操作使用显式目标更清晰，也避免把本地临时状态引入项目语义。

## 4. Cursor 迁移规则

一次 mutation 记为作用于 path `M`，当前 cursor path 为 `C`。所有规则在旧树坐标上计算，保存 snapshot 成功后再持久化新 cursor。

### 4.1 不改变层级的操作

- `replace` / `rewrite` / `search-replace`：`C == M` 时 cursor 保持 `M` 并刷新 fingerprint；若 `M` 是 `C` 的祖先，任意 replacement 无法证明内部节点对应关系，cursor 降级到 `M` 并提示；其他位置不变。
- `replace-leaf`：不改变路径；若恰好替换 cursor 节点，刷新 fingerprint。
- `append-child`：已有节点路径不变。

### 4.2 插入

- `insert-before M`：与 `M` 同父且 index 大于等于插入点的 cursor，index 加一；cursor 位于这些兄弟节点的 subtree 内时同样迁移。
- `insert-after M`：同父且 index 大于 `M` 的 cursor，index 加一。
- `insert-child M`：cursor 是 `M` 的严格后代时，紧随 `M` 后的第一段 index 加一；cursor 正好在 `M` 时不变。

### 4.3 删除与重排

- `delete M`：位于 `M` 之后的同级 cursor index 减一；`C == M` 或 `C` 位于其内部时，cursor 移到 `M` 的 parent 并返回 `target-deleted` 提示。
- `batch-delete`：按实际从后向前的删除顺序逐条应用同一规则。
- `swap-next M` / `swap-prev M`：cursor 位于两个交换 sibling 任一 subtree 时，交换对应 path 段。
- `unwrap M`：cursor 在 wrapper 内时移除 `M` 对应的 wrapper path 段，并叠加 child index；cursor 在其后的 sibling 时按展开 child 数调整。
- `raise M`：cursor 位于被提升 child 内时删除 parent 到 child 的 path 段；位于被丢弃 sibling 内时降级到被替换的 parent。
- `wrap M`：`C == M` 时选择 wrapper；若 cursor 位于原 subtree 内，只有能从模板中的唯一 `self` 映射证明新路径时才跟随，否则降级到 wrapper。

所有迁移完成后必须用新 snapshot 重新读取 cursor path，刷新 fingerprint 和 definition revision。preview 每次从当前 Snapshot 构造，不持久化到 cursor history。新 path 无法读取时不写一个看似有效的 cursor。

## 5. 外部变化与 stale 恢复

`calcit cursor show` 和 `--path @cursor` 每次都重新解析 snapshot：

1. revision 与 fingerprint 均匹配时状态为 `exact`；
2. revision 变化但 path fingerprint 仍匹配时刷新 revision，状态为 `verified-at-path`；
3. path 不匹配时，全 definition 搜索旧 fingerprint；唯一命中则重定位并提示 `relocated`；
4. 零命中或多命中时拒绝作为 mutation target，报告命中数量并要求重新 set。

自动恢复必须宁可失败，也不能在重复结构中猜测。

## 6. Edit 命令、Transaction 与并发

`edit cp/mv/split-def` 的 path 参数接受 `@cursor`。`edit def --overwrite` 把 cursor 安全降级到 definition root；`rename` / `mv-def` 更新 target；`split-def` 在 cursor 位于被抽取 subtree 内时把 target 切换到新 definition，并保留相对 path。`cp` 使用与 tree insertion 相同的确定性坐标迁移；`mv` 完成后按 fingerprint 验证或唯一重定位。

定义被 `rm-def` 删除时不删除整个 sidecar，因为 history、stack 和 clipboard 仍可能用于恢复。active cursor 被明确标为 stale，并提示使用 `cursor back` 或重新 `set`。

transaction 的 staged 子命令不得直接更新真实 cursor 文件。完整目标是 transaction 开始时读取 cursor 到内存，按 operation 顺序迁移 staged cursor；只有 snapshot 原子提交成功后才写 cursor，dry-run 返回 `cursor_before` / `cursor_after` 但不更新文件。

当前实现先满足较保守的兼容路径：staged 子命令禁用 cursor sidecar 写入；transaction 提交后重新解析最终 snapshot，并用 same-path fingerprint 或全 definition 唯一 fingerprint 验证 active cursor。无法唯一恢复时只提示 cursor 需要处理，不把已经成功提交的 transaction 误报成回滚。逐 operation 的 staged cursor 迁移以及 dry-run 的 `cursor_before` / `cursor_after` 仍是后续增强。

`cursor cut` 与 `cursor paste` 同时影响 Snapshot 和 sidecar，两个文件不能由普通 rename 构成单一原子事务。实现必须先把两边都写入同目录 staged 文件，再按可恢复顺序提交：cut 先提交含完整 clipboard 的 sidecar，再提交删除后的 Snapshot，保证后一步失败时表达式仍可恢复；paste 先提交 Snapshot，再提交新 cursor，并在第二步失败时明确报告“源码已修改”，禁止把它伪装成可安全重试的普通失败。

snapshot 与 cursor 是两个文件，无法依靠单次 rename 同时提交。若 snapshot 已成功而 cursor 写入失败，后续调用会通过 revision/fingerprint 检出 stale；错误必须明确报告“源码已提交、cursor 未更新”，不能声称整体回滚。

当前状态只有一个 active cursor。原子 rename 保证文件完整性，但不提供多个进程共享同一 Snapshot 时的语义并发控制；并行 Agent 应使用独立 worktree/Snapshot，或改用带 precondition 的 transaction 与显式路径。marks 只能隔离导航位置，不能解决并发源码写入，因此不作为并发安全方案。

兼容性更新（2026-08-14）：CLI 仍只操作 `:active` 指向的 cursor，但 cursor 文档读写会保留该名称以及 `:cursors` map 中其他合法条目，不再在保存 active state 时把 map 收缩为固定的 `:main`。新建 sidecar 的默认名称改为 `:default`，读取旧 `:main` 不受影响。这只为未来 cursor user 排除有损迁移障碍，尚不提供 user 选择、lease、心跳或并发写保护。

### 6.1 下一阶段：cursor user

CLI 将引入项目本地 `cursor user`，取代进程间共享的 active selection：

```bash
calcit --cursor-user agent-a cursor show
CALCIT_CURSOR_USER=agent-b calcit tree show @cursor --path @cursor
calcit cursor show # 未指定时使用 default
```

解析优先级为 `--cursor-user` > `CALCIT_CURSOR_USER` > `default`。`@cursor`、history、stack、anchor、marks、last-query 和 clipboard 全部按 user 隔离；`cursor whoami/users` 提供发现接口。

后续持久化推荐使用 `.calcit/cursors/<user>.cirru`，而不是让多个进程 read-modify-rename 同一个 `:users` map；否则即使文件写入原子，仍会丢失另一个 user 的并发导航更新。`cursor users` 通过目录发现，v1-v4 的 active entry 迁入 `default` 文件，其他合法 named entry 迁入对应 user 文件。

source mutation 只立即迁移发起 user 的 cursor/anchor/marks；其他 user 下次访问时按 definition revision、same-path fingerprint 或唯一 fingerprint 懒惰恢复，无法恢复则标 stale。cursor activity、lease 和 overlap warning 不进入第一版；任务重叠由 orchestrator 比较 work item write-set，写入正确性依赖 Snapshot/definition revision precondition。完整边界见 `08-14-architecture-scaffold-rfc.md` 第 7 节。

## 7. 第一阶段实现范围

1. Cirru EDN cursor 状态的读取、v1-v3→v4 兼容、旧路径一次性迁移、校验、64 KiB 上限和原子写入；
2. `calcit cursor set/show/clear/parent/child/next/prev/forward/backward/back/push/pop`，含末子节点、同级多步与跨 list 的深度优先导航；
3. 结构化 `copy/cut/paste/clipboard`，clipboard 不经过文本序列化；
4. definition-oriented query/tree/edit 接受 `@cursor` target；tree/type-at/edit 的 path 可同时引用 `@cursor`；
5. `edit cp/mv/split-def` 接受 `@cursor`，definition replace/rename/move/delete 与 cursor 协作；
6. 所有直接 tree mutation 对 active cursor 执行确定性 path 迁移，并按 `--cursor-after` 控制回显；
7. 外部 revision 变化下的 same-path 校验与唯一 fingerprint 重定位；
8. 单元测试覆盖前方插入、前方删除、swap、目标删除、状态扩展字段、clipboard round-trip 和 focus 展示不影响真实 path。
9. `query search` / `query search-expr` 为结果提供全局 cursor index，并可显式设置 active cursor。
10. `cursor apply` 复用 tree mutation；双向 slurp/barf 与 duplicate 保持 active selection；
11. leaf/expression search 都支持 `--filter @cursor` 与 `--start-path @cursor`，并继续支持 `--set-cursor`。
12. 单一 sibling region anchor、最多 16 个 named marks，以及只保存参数的 last query / `query next/prev`。

transaction 内逐 operation 的 cursor preview、多 cursor user、region 批量 mutation，以及跨 definition 的 clipboard 引用策略在后续阶段接入；未接入的 mutation 必须让 cursor/anchor/marks 在下次使用时经过 stale 校验，不能绕过 fingerprint。

## 8. 验收

- cursor 位于 `@3.4` 时在同父 `@3.2` 前插入，自动变为 `@3.5`；
- cursor 之前的 sibling 被删除时自动减一；
- 对 cursor 之前的 subtree 做内部修改，不改变 cursor；
- cursor 自身被删除时移动到 parent 并给出提示；
- cursor 展示 wrapper 不改变真实 path 或 JSON tree；
- focus 展示保留 definition 签名，并对无关分支使用结构化 folded marker；
- `back` 与 `push/pop` 语义独立，clipboard 的 cut/paste 保持 Cirru subtree；
- `child --last`、`next/prev/forward/backward/back --count N` 在成功时只产生一条 history 记录，越界时不修改 cursor；
- `cursor apply` 与对应 tree 命令的 mutation、校验和 preview 一致；
- 双向 slurp/barf 跨 parent 边界移动表达式后 cursor 仍选择原 list；duplicate 选中新副本且不修改 clipboard；
- query/tree/edit 的 `@cursor` target、type-at path 与 subtree search scope 解析到同一 active selection，target 冲突时拒绝执行；
- search human/JSON 中的 cursor index 一致，`--set-cursor N` 不污染 JSON stdout；
- `query next/prev` 不保存结果列表，每次重新计算；snapshot 改变后拒绝复用旧 index；
- region 只接受同 parent 的连续 siblings；marks 不超过 16 个，并随可证明的 tree path 变换迁移；
- `--cursor-after none|summary|focus` 只改变 stderr 反馈，不改变 mutation 结果；
- snapshot 被外部修改后，重复 subtree 不会被猜测性选中；
- `.calcit/cursor.cirru` 始终是可由 `cirru_edn::parse` 读取、且不超过 64 KiB 的单个 EDN value；
- cut 的 Snapshot 提交失败时 clipboard 已可恢复；paste 的 cursor 提交失败时必须明确报告 Snapshot 已修改。
