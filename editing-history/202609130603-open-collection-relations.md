# 收敛开放集合的普通泛型关系

时间：2026-09-13 06:03 +0800

## 背景

#995 的生态复现包含 `conj`、`assoc`、`map-indexed` 与 `=`。当前实现已经允许 `conj`/`assoc` 把具体值
安全放入显式开放容器，但 `map-indexed` 没有从 receiver 派生 callback 类型，`=` 的泛型绑定还会因
Dynamic 实参位于首位或后位而得到不同结果。

## 调整

- 为 `map-indexed` 增加 receiver-bound checked-call contract，从 `List<T>` 派生 `(Number,T)->U` 和 `List<U>`。
- 泛型关系验证优先处理包含 Dynamic 的实参，再验证具体实参，消除同一关系的参数顺序差异。
- 保留严格边界：`List<Dynamic>` 仍拒绝只接受 Number 的 callback，并继续报告 `E_ERASED_GENERIC_RELATION`。
- 在 `assoc`、`conj`、`map-indexed` 与 `=` definition 下增加带 `:core :unit` tag 的 Calcit-first 测试。
- 在现有 Calcit traits 测试入口补充开放列表的双向相等与 `map-indexed` 断言，让 native/生成 JS 共用同一组语义样例。
- 不增加 `*-dynamic` API、analyzer 或统计门禁。

## 验证

- 完整 Rust 测试通过（773 lib、340 CLI、17 WASM、4 fix CLI 等）。
- core definition 主入口通过 261 项，并以 `--tag unit --require-match` 选中新增测试。
- 默认严格预处理下，开放 `map-indexed` 与双向 `=` 实际执行通过；Number-only callback 的拒绝用例保持失败关闭。
- `yarn check-all` 全部通过，覆盖 native、生成 JS、Agent CLI、WASM 实际执行和现有性能回归入口。
