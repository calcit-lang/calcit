# Preserve Syntax collection lowering / 保留 Syntax 集合降级

- Keep the existing `map`/`filter` static lowering for concretely proven
  `List<Syntax>` and `Set<Syntax>` receivers while leaving their callback
  checking evidence open. This compatibility case now lives in the unified
  contract instead of restoring a lowering-side name table.
- 对已证明的 `List<Syntax>` 与 `Set<Syntax>` receiver 保留既有 `map`/`filter`
  静态 lowering，同时不收紧 callback checking evidence；该兼容分支归入统一
  contract，不重新引入 lowering 侧名称匹配表。

## Validation / 验证

- `cargo test checked_call_contract`
- `cargo test collection_callback_specialization_skips_syntax_members`
- `cargo test specializes_public_map_for_set_receivers`
- `cargo test specializes_public_filter_to_shape_preserving_core_definitions`
