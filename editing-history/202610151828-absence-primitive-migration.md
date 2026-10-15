# 缺失值原语的受保护迁移

## 依据与版本范围

十三个底层缺失值原语已由 nil-or-T 改为 Option；公开 API 则包含 Option、Result 和集合
Destruct 三种合同。维护者确认这项原计划 0.31.0 的变化提前进入 0.30.1，沿用 main 开发与
发布，不建立维护分支。其他历史类型清理和弃用入口退场不提前。

## 决策与边界

- 复用现有 fix、resolver/source evidence、附带区域 planner、revision 和严格暂存校验。
  只有一条显式迁移桥，不增加顶层命令、默认 preset、长期 analyzer 或新公共 API。
- 自动范围限定为输入域已证明、直接使用旧 nil?/non-nil?/either 的调用；原语来源必须由
  reader 或同一源码位置的全部 resolver 证据确认。未知域、间接绑定、一等函数、宏模板、
  旧 arity 保留人工审阅，不猜业务默认值、不增加 Dynamic 或 unsafe。
- 匹配公开返回合同恢复原消费语义，Some nil 不当作缺失。解析匹配 Result，集合解构匹配
  MapDestruct/SetDestruct，不套用单 payload 的 Option。
- either 外层保留各 fallback 的原始语法一次；不改成 eager unwrap-or，也不生成 thunk
  或复制 fallback，避免改变副作用、报错和闭包身份。
- 字符串负下标是实际的失败语义差异：旧原语报错，公开 nth 可能返回 none。因此只自动
  处理有限、非负、i32 范围内的整数字面量，其他下标需要审阅。
- 暂存阶段不沿用迁移规划的宽容模式；同一 scope 还有未迁移的非法源码时，整批不写回。
  quoted data 不扫描；tests/examples 使用同一证据入口，但不修改断言或标签。
- 活跃消费者与依赖的源码、附带区域、宏生成代码全部迁移验收后，在下一个非 patch
  版本审阅退役这条桥。原语已合并不代表消费者验收完成。

## 验证方式

`tests/fixtures/absence-migration.cirru` 通过 CLI 的 dry-run / scoped revision 事务创建。
25 项 definition :tests 覆盖全部十三个原语、Some nil / None / false / zero / 空字符串、
解析失败、空/非空集合解构、Unicode 访问、fallback 效果与单次求值。

同一旧源码使用正式 CLI / @calcit/procs 0.30.0-alpha.5 配对，在既有跨后端运行器的 native / JS
各 25/25 通过；候选迁移应用到临时副本后，当前同源 CLI / TS runtime 在 native / JS 也各
25/25 通过，无排除项。Rust 只检查 CLI 的预览、revision、
确定性、原子写回、review-only/no-op 和暂存失败保护；不在 Rust 重写上述语言语义断言。

这些结果不是正式 CLI/runtime 配对发布，也不代替 #2080 中九个真实消费者的验收。
