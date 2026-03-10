# 2026-02-14 20:50 JS codegen recursion + tag migration

## 背景

在 trait/macro 改进后，继续针对 JavaScript 编译目标做性能与可维护性优化。

## 本次改动

1. 参数个数报错模板统一

- 在 runtime 新增 `_args_fewer_throw` / `_args_between_throw`，保留 `_args_throw`。
- emitter 中 `too few/too many` 模板改为调用统一 helper，错误信息继续携带期望与实际参数个数。

2. rest 参数处理优化

- 对 `&` rest 参数，仅在函数体实际引用时才注入 `arrayToList` 转换，避免不必要转换。
- 修复符号引用检测边界：`contains_symbol` 增加 `Calcit::Local` 分支。

3. tail recursion 模板优化

- watchdog 从每轮检查改为周期检查（`(times & 1023) === 0` 时再判断上限）。
- 对无 rest/optional 的固定参数函数，recur 回填改为索引赋值（`arg = ret.args[i]`），减少解构开销。

4. tag 初始化一次迁移

- runtime 新增 `init_tags` 与全局 tag 缓存。
- emitter 统一改为 `const _t_ = init_tags([...])` / `$clt.init_tags([...])`。
- 移除每模块内联 `forEach + newTag` 初始化模板。
- 修复 `calcit.core` 目标缺失导入，补上 `init_tags` import。

## 验证

- `yarn check-all` 通过。
- 额外执行了 JS 微基准（`test-recursion.main/test_loop`）用于观察 recursion 模板优化影响。

## 经验点

- JS 生成模板优化要先保证语义完整，再做结构性收敛（helper 化/去重复）。
- 对 codegen 的性能优化，建议配套微基准，并与 baseline 做同脚本对比。
