# Bound recursive type analysis / 有界递归类型分析

Issue: calcit-lang/calcit#845

## English

- Added an iterative, pair-memoized relation path for deeply nested lists,
  maps, options, references, nominal arguments, and shared type DAGs. A fixed
  16,384-node budget returns an explicit complexity boundary instead of
  authorizing a Dynamic fallback.
- Made nominal data-schema identity traversal iterative and bounded. Protected
  schema-alias expansion with relation-local cycle state so mutually recursive
  named types remain valid while pure alias cycles are rejected explicitly.
- Bounded diagnostic type rendering, and made generic substitution/type-variable
  scans iterative. Substitution caching remains inside one call so shared
  subgraphs stay shared without leaking bindings across entries or reloads.
- Added a read-only field-index cache for structs with at least 32 fields. It
  keys live immutable field allocations with `Weak` validation, preserves
  declaration and first-match order, and does not reuse stale reload entries.
- Added fixed depth 32/256/2048, shared-DAG, recursive Tree/Node, alias-cycle,
  complexity-limit, diagnostic-bound, and width 32/256/1024 regressions. The
  measurement tests report visits, worklist size, tracked payload memory,
  allocator availability, and cold/warm time.
- Validation: `cargo fmt`, 763 library tests, and clippy with all targets and
  features passed. Calcium's generated-JavaScript smoke oracle passed all
  no-op sharing, patch convergence, corruption, revision, and recovery checks.
  Recompiling current Calcium with the new strict frontend independently finds
  its existing `Option<dynamic>` generic boundary; this is recorded separately
  from the #845 relation regression.

## 中文

- 为深层 List、Map、Option、Ref、名义类型参数及共享类型 DAG 增加显式 worklist
  与成对访问记忆。固定 16,384 节点预算，超限时返回明确的 complexity boundary，
  不允许借 Dynamic 静默放宽。
- 将名义数据 schema identity 比较改为迭代且有界；schema alias 展开使用关系局部
  的环状态，使合法具名互递归继续成立，同时明确拒绝纯 alias cycle。
- 限制诊断类型渲染长度，并将泛型替换与类型变量扫描改为迭代实现；替换结果只在
  单次调用内缓存，保持共享子图，且不将绑定泄漏到其他 entry 或 reload。
- 为至少 32 个字段的宽 Struct 增加只读 field-index cache。缓存通过 `Weak` 校验
  不可变 fields allocation，保留声明顺序与首个匹配语义，也不会复用失效的 reload
  entry。
- 增加固定深度 32/256/2048、共享 DAG、递归 Tree/Node、alias cycle、复杂度超限、
  诊断截断以及宽度 32/256/1024 回归。度量测试记录访问数、worklist 大小、已跟踪
  payload 内存、allocator 可用性与 cold/warm 时间。
- 验证：`cargo fmt`、763 项库测试及 all-target/all-feature clippy 已通过。Calcium
  生成 JavaScript 的 smoke oracle 通过 no-op 共享、patch 收敛、损坏检测、revision
  与恢复全部断言。用新严格前端重新编译当前 Calcium 会独立暴露其既有
  `Option<dynamic>` 泛型边界；该结果与 #845 类型关系回归分开记录。
