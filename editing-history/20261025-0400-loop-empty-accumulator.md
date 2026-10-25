# 空 loop 累加器从 recur 传入值求解成员类型

## 问题与依据

#2279 之后，返回声明为 `List 'T` 的函数里，`loop` 累加器初值为 `([])`、由 `recur` 逐步 `&list:append` 成员时，普通检查报 `E_ERASED_GENERIC_RELATION`（`expected list<'T>, got list`）。`recollect.diff/set-difference-dynamic` 因此失败；0.31.0-alpha.2 通过。初值写成 `[] x` 时正常。

递归输入求解器本应用 recur 传入值求出空槽类型，但实际得到 `List Dynamic`，以前只是被宽松的返回检查放过。两处把 Dynamic 混入了空槽：

1. 准备阶段 `&list:append` 的 proc 参数是开放的 `List Dynamic`，调用约束把空槽直接绑成 Dynamic。
2. `if` 分支合并把尚未求解的空槽变量当作具体类型，与另一分支的 `List 'T` 合并成 Dynamic。

## 修复边界

只在函数体确有 tail transfer 时，开放 proc 参数不作为 loop 空槽的证据；成员类型由集合增长推断和 recur 传入值给出。没有 recur 的回调（如 `fold` 的空种子回调）仍按原路径求解，其开放行为不变。分支合并中，正在求解的 loop 空槽按 Never 处理，与原有空容器合并规则一致。

真实的冲突仍被拒绝：追加 String、混入 Number、或经 Option 取值后的成员，都无法证明为 `'T`。不新增诊断、规则或 Dynamic。

## 验证

`test-types-inference` 新增 `keeps-element-binder-of-loop-accumulator`：原二进制失败，新二进制 native / JS / WASM 通过。`check-known-assertion.mjs` 在现有泛型输出拒绝表中加入两个 loop 用例（具体成员、混合成员），经普通检查、concrete-return 规则和 strict 拒绝。首版修复让 core 的 `fold#left-to-right` 变红，原因是无 recur 的回调也走同一求解器，现已限定到 tail transfer。
