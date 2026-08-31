# Revision-safe Calx compile cache design / revision-safe Calx 编译缓存设计

Tracking: [calcit#552](https://github.com/calcit-lang/calcit/issues/552) and [calx-vm#39](https://github.com/calcit-lang/calx-vm/issues/39)

## 中文

### Profile 结论

基于干净提交 `ef04017f0bc60e2d8f6341599f00d426a11ed360` 的 release profile 见
[`20260831-compile-profile-macos-arm64.json`](../../benchmarks/calx/20260831-compile-profile-macos-arm64.json)。
五个 scalar kernel 的未插桩完整编译约为 15.3–20.9 μs/次；分阶段测量中 program construction 占总时间
约 47–54%，eligibility 与 planning 各约 17–23%，validation/lowering 约 5–7%。每次编译产生约
222–392 次 allocation、52–91 次 reallocation 和 22–38 KiB 累计请求。

Samply 的 20,987 个 CPU samples 将主要 inclusive 路径定位到 `emit_expression`（44.3%）、
`source_origin`（35.6%）和 `BodyBuilder::if_else_at`（34.4%）。8,018 个包含 allocator frame 的 samples 中，
62.4% 经过 `emit_expression`，53.8% 经过 `source_origin`。inclusive stack 会重叠，不能把百分比相加；但阶段
计时和调用栈共同说明，首个高价值切片应缓存完整、已经验证的 source-derived artifact，而不是只微调
`ProgramBuilder::build`，也不是先做 VM pool。`source_origin` 的字符串构造仍值得作为 cache miss 路径的
后续独立优化。

### 对象边界

首版把当前 `CalxCompiledKernel` 拆成两层：

- `CalxCompiledArtifact`：不可变、可缓存，只持有 eligible graph、entry boundary、`ValidatedProgram`、
  reachable definition stamps、Calx ABI edition 与 typed import declaration contract；
- `CalxPreparedKernel`：每次调用产生，持有一个 artifact 引用和本次 embedding 提供的
  `CalxHostBindings`；运行或建立 fresh VM 只通过这一层。

callback、capability state、buffer input、VM instance、stack/frame 和 runtime trap 都不进入 artifact。
缓存命中也必须重新校验 typed import declarations，并从本次 `CalxHostImports` 重新附着 callbacks。

### Revision key 与命中算法

缓存由 embedding 显式创建并传入，不使用进程级 singleton。顶层 slot key 包含：

1. fully-qualified entry；
2. Calx kernel ABI edition；
3. 排序后的 typed import declaration contract（definition、export name、参数与结果类型），不含 callback identity。

每个 artifact 另外保存排序后的 reachable definition stamps。当前 `CompiledDef::version_id` 尚未提供可用的
内容 revision（现有路径仍写入 `0`），因此首版不能把它单独当作 correctness key。stamp 必须保存并比较：

- `DefId` 与 fully-qualified definition；
- `preprocessed_code` 的结构值；
- `schema` 的结构值。

Calcit 与 type annotation 都已有一致的 `Eq`/`Hash`；hash 只用于加速候选查找，最终命中必须做结构相等
比较，不能让 hash collision 变成 stale hit。`Calcit` collection clone 复用持久化结构，stamp 不需要复制整份
source store。以后若 `CompiledDef` 获得“内容未变则稳定、内容或 schema 变化则递增”的真实 revision，才可用
它替代结构 stamp。

lookup 顺序：

1. 用 slot key 查找 candidate；
2. 逐个从当前 `CompiledProgram` 读取 candidate 已记录的 reachable definitions，并比较 stamp；
3. entry 或任意旧 callee 缺失、代码/schema 改变时 miss；entry 改变会触发重新编译并发现新的 call graph；
4. 未出现在旧 reachable set 的无关 definition 不参与比较，因此保持 hit；
5. 命中后使用本次 imports 重新建立 bindings，并再次检查声明签名；只有完成 attachment 才返回 prepared kernel。

这是一种 dependency-validation cache，而不是“对整个 Snapshot 求 hash”。它既覆盖 transitive change，也不会让
无关 definition 的热更新冲掉全部 Calx artifact。

### API、容量与观测

公共能力优先以对象方法提供。建议的实验接口为：

```text
CalxCompileCache::new(capacity)
cache.prepare(program, namespace, definition, imports)
cache.stats()
cache.clear()
```

`capacity` 首版按 artifact 个数设置硬上限，使用确定性的 LRU eviction；不得无界增长。stats 至少包含 hit、
miss、miss reason、eviction、clear、entry count、reachable function count、syntax/instruction count 与估算字节数。
miss reason 固定区分 `empty`、`entry-changed`、`callee-changed`、`schema-changed`、`abi-changed`、
`import-contract-changed`、`dependency-missing` 和 `evicted`。命中报告明确标记跳过 eligibility、planning、
program construction、validation/lowering，但仍记录 revision validation 与 binding attachment 成本。

为了让 `evicted` 可判定，cache 维护独立的 recently-evicted slot-key ledger。ledger 保存完整 slot key，容量不超过
artifact capacity，并按确定性的 LRU 顺序裁剪；它不是 artifact，也不保存 program、callback 或 definition stamp。
淘汰 artifact 时插入或刷新其 key；再次插入同 key 时移除 tombstone；`clear()` 同时清空 artifact 与 ledger。
当 active slots 中没有 key 时，ledger 命中报告 `evicted`，否则报告 `empty`。artifact 淘汰当下只增加
`evictions`，后续实际 lookup 才增加 `misses.evicted`，避免一次淘汰重复计为 miss。ledger 自身淘汰旧 key 不增加
artifact eviction 统计，之后该旧 key 按 `empty` 处理。

### 安全与测试门槛

- hot reload 改 entry、direct/transitive callee 或 schema 必须 miss；无关 definition 改动必须 hit；
- callback A 编译后，以相同声明传 callback B 命中缓存，执行必须只调用 B；
- import declaration 改动必须 miss，callback identity 改动本身不能污染 source-derived key；
- capacity、LRU eviction、显式 clear 与 stats 必须可测试；
- eviction provenance 必须覆盖 capacity=1 下插入 A、插入 B、查询 A 得到 `evicted`；ledger 溢出后最旧
  tombstone 查询得到 `empty`；重新插入或 clear 后旧 tombstone 不得继续报告 `evicted`；
- partial lowering、placeholder 和 negative eligibility result 不缓存；
- eligibility fallback 仍只发生在执行前，runtime trap 后不重跑 Calcit；
- benchmark 必须同时报告 uncached one-shot、cache hit + fresh VM、cached-native 与 reused-Calx execution，
  并保留 allocation/estimated-memory 成本。

首版不包含 VM pool、自动 offload、持久化磁盘 cache、persistent collection 或 typed-buffer 扩展。

---

## English

### Profile findings

The release profile from clean commit `ef04017f0bc60e2d8f6341599f00d426a11ed360` is recorded in
[`20260831-compile-profile-macos-arm64.json`](../../benchmarks/calx/20260831-compile-profile-macos-arm64.json).
Uninstrumented complete compilation takes approximately 15.3–20.9 μs per scalar kernel. In staged measurements,
program construction accounts for approximately 47–54% of total time, eligibility and planning each account for
approximately 17–23%, and validation/lowering accounts for approximately 5–7%. One compilation performs roughly
222–392 allocations, 52–91 reallocations, and requests a cumulative 22–38 KiB.

Across 20,987 Samply CPU samples, the main inclusive paths are `emit_expression` (44.3%), `source_origin` (35.6%),
and `BodyBuilder::if_else_at` (34.4%). Of 8,018 samples containing an allocator frame, 62.4% pass through
`emit_expression` and 53.8% pass through `source_origin`. Inclusive stacks overlap and their percentages must not be
summed. Together, the stage timings and call stacks show that the first high-value slice is caching the complete
validated source-derived artifact, not only tuning `ProgramBuilder::build` and not premature VM pooling.
`source_origin` string construction remains a separate follow-up for cache-miss performance.

### Object boundary

The first implementation splits the current `CalxCompiledKernel` into two layers:

- `CalxCompiledArtifact`: immutable and cacheable; owns the eligible graph, entry boundary, `ValidatedProgram`,
  reachable-definition stamps, Calx ABI edition, and typed-import declaration contract;
- `CalxPreparedKernel`: created per request; holds an artifact reference and the `CalxHostBindings` supplied by the
  current embedding call. Execution and fresh-VM construction are available only through this layer.

Callbacks, capability state, buffer inputs, VM instances, stack/frame state, and runtime traps never enter the
artifact. A cache hit must revalidate typed import declarations and attach callbacks again from the current
`CalxHostImports` value.

### Revision key and hit algorithm

The embedding creates and passes the cache explicitly; there is no process singleton. The top-level slot key
contains:

1. the fully qualified entry;
2. the Calx kernel ABI edition;
3. the sorted typed-import declaration contract (definition, export name, parameter types, and result type), excluding callback identity.

Each artifact also stores sorted reachable-definition stamps. `CompiledDef::version_id` is not yet a usable content
revision because current paths still write `0`, so the first implementation must not use it as the sole correctness
key. A stamp stores and compares:

- `DefId` and the fully qualified definition;
- the structural value of `preprocessed_code`;
- the structural value of `schema`.

Calcit values and type annotations already provide consistent `Eq` and `Hash`. A hash may accelerate candidate
lookup, but a final structural equality check is required so a collision cannot create a stale hit. Cloning Calcit
collections shares their persistent structure, so stamps need not duplicate the complete source store. A future
`CompiledDef` revision may replace structural stamps only after it remains stable for unchanged content and advances
for every code or schema change.

Lookup proceeds as follows:

1. find a candidate by slot key;
2. read every previously reachable definition from the current `CompiledProgram` and compare its stamp;
3. miss when the entry or any old callee is missing or its code/schema changed; an entry change recompiles and discovers the new graph;
4. ignore definitions outside the old reachable set, preserving a hit for unrelated hot reloads;
5. on a candidate hit, rebuild bindings from the current imports and check declaration signatures again; return a prepared kernel only after attachment succeeds.

This is a dependency-validation cache, not a hash of the whole Snapshot. It covers transitive changes without
invalidating every Calx artifact for an unrelated definition update.

### API, capacity, and observability

Public capability should be method-oriented. The proposed experimental surface is:

```text
CalxCompileCache::new(capacity)
cache.prepare(program, namespace, definition, imports)
cache.stats()
cache.clear()
```

The first `capacity` is a hard artifact-count limit with deterministic LRU eviction; growth is never unbounded.
Stats include hits, misses, miss reasons, evictions, clears, entry count, reachable function count,
syntax/instruction count, and estimated bytes. Stable miss reasons distinguish `empty`, `entry-changed`,
`callee-changed`, `schema-changed`, `abi-changed`, `import-contract-changed`, `dependency-missing`, and `evicted`.
A hit reports that eligibility, planning, program construction, and validation/lowering were skipped while still
recording revision-validation and binding-attachment cost.

To make `evicted` observable, the cache keeps a separate recently-evicted slot-key ledger. It stores complete slot
keys, is bounded to at most the artifact capacity, and is trimmed in deterministic LRU order. It is not an artifact
and retains no program, callback, or definition stamp. Evicting an artifact inserts or refreshes its key; reinserting
that key removes its tombstone; `clear()` clears both artifacts and the ledger. When no active slot matches, a ledger
hit reports `evicted`, otherwise the miss is `empty`. The eviction itself increments only `evictions`; a later lookup
increments `misses.evicted`, avoiding double-counting. Removing an old key from the bounded ledger does not count as
an artifact eviction, and a later lookup of that forgotten key reports `empty`.

### Safety and test gate

- Hot reloads of the entry, a direct/transitive callee, or a schema must miss; an unrelated definition change must hit.
- After compiling with callback A, a hit with the same declaration and callback B must execute only B.
- Import declaration changes must miss; callback identity itself must not contaminate the source-derived key.
- Capacity, deterministic LRU eviction, explicit clear, and stats must be covered.
- Eviction-provenance coverage inserts A then B at capacity one and expects an A lookup to report `evicted`; after
  the ledger overflows its oldest tombstone reports `empty`; reinsertion and clear must not leave a stale `evicted`
  reason.
- Partial lowering, placeholders, and negative eligibility results are never cached.
- Eligibility fallback remains compile-time only; a runtime trap never reruns Calcit.
- Benchmarks compare uncached one-shot, cache hit plus fresh VM, cached-native, and reused-Calx execution while retaining allocation and estimated-memory costs.

The first slice does not include VM pooling, automatic offload, persistent disk caching, persistent collections, or
typed-buffer extensions.
