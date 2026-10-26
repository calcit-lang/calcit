# WASM 用 impl selector 携带多个名义实现身份

#2339 的剩余部分：WASM 值被擦除成 f64，Enum 堆布局没有 impl 身份，方法分派只能依赖静态证据。
以下三种情况里，同一个局部量可能在运行时持有两个不同实现身份的名义值，旧代码要么拒绝编译，要么退回按合同
分派（open Enum 上会丢失用户定义的 `.contains?`，即 #2385 的回归形态）：

- `if` 两个分支给出不同的 exact impl（let 绑定、调用参数位置）；
- 特化 `recur` 循环在回边换成另一个实现身份；
- 带 `recur` 的内联闭包和 `loop`（IIFE）。

修改（`src/wasm/emit_wasm.rs`，不改 ABI）：

- 值局部量旁边加一个 i32 selector，记录它当前是有限个 exact 实现身份中的哪一个（最多 8 个）。
- 读取带 selector 局部量的调用按每个身份拆成 if 链，每个分支看到确定的 exact 证据；不会按 Enum 种类或
  Bool 返回合同挑默认实现。
- `if` 汇合在 let 或参数位置时分配 selector，分支内写入对应编号。
- 特化 recur 循环在回边发现新身份时用内部标记重试，扩大每个参数的身份集合，并在回边写 selector；
  超过上限或证据缺失时报 `E_WASM_CALL_SPECIALIZATION`。
- 固定参数的内联闭包遇到 `recur` 时用参数证据运行同一个选择循环；`loop` 在初值带名义 exact 证据时也走
  选择循环，回边没有名义证据则退回原来的按合同循环，已能编译的代码行为不变。

回归：`calcit/test-traits.cirru` 新增带 `open-branch-impl-evidence` 与 `recur-closure-impl-evidence`
标签的定义测试，均带 `impl-origin-portable`；CI 中原来回放 `recur-impl-evidence`（其子集）的那一步改为
回放 `impl-origin-portable`。`test-collection-traits` 的 `changed-impl`、`parallel-swap-changed-impl`
在 WASM/WASI 上也由编译失败变为通过。

English: WASM locals that may hold one of several exact nominal impl identities now carry an i32
selector, and calls on them dispatch per identity. Open `if` joins, specialized `recur` loops whose
back edges change identity, recursive inline closures and `loop` forms use it instead of rejecting or
falling back to contract dispatch.

补充：参数位置的汇合只把汇合之前的非原子实参先绑定，原子实参保持原位，因为构造器等调用按语法读取它们（如
`&%{}` 的 struct 引用）；`test-traits.main/branch-open-payload` 以直接构造器覆盖 struct 与 Option 的汇合载荷。`loop` 的退回路径不吞掉超过 8 个实现身份的诊断。
固定参数内联闭包的 `recur` 现已支持，所以 `scripts/check-known-assertion.mjs` 改为运行该闭包的 WASM 产物，
`calcit.core/foldl-compare#typed-number-exits` 移出 WASM/WASI 排除表，`docs/installation/wasm-support.md`
同步支持状态。
