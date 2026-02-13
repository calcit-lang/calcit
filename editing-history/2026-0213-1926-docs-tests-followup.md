# 2026-02-13 19:26 文档与测试补充（跟进）

## 背景
在完成 trait/macro 相关优化（P1/P2/P3）后，补做“是否需要补充测试与文档”的审计。

## 本次结论
需要补充，且已补最小必要范围：

1. 文档：
   - `guidebook/docs/features/macros.md`
     - 增加 `with-gensyms` 用法示例。
     - 增加 `macroexpand`/`macroexpand-1`/`macroexpand-all` 的展开链路说明（stderr）。
   - `guidebook/docs/features/traits.md`
     - 增加 `warn-dyn-method` 下的新增诊断说明。
     - 明确 `&trait-call` 按 impl 的 trait origin 匹配，而非仅按名称文本匹配。

2. 测试：
   - `src/calcit/calcit_trait.rs`
     - 新增 default identity 单测：
       - `def_ref` 相同 => trait 相等且 hash 一致；
       - 无 `def_ref` 时走函数元信息 fallback，并校验等价/不等价边界。

## 验证
- `cargo test -q calcit_trait::tests` 通过。
- `yarn check-all` 通过（本轮补丁后再跑）。

## 经验点
- 文档应及时反映“诊断开关”边界（默认流程 vs `warn-dyn-method`）。
- identity 规则变化必须配对 hash/eq 回归测试，避免后续 refactor 引入行为漂移。
