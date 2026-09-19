# JS FFI 迁移盘点输出 defexternal 骨架（#1224 阶段一）

- `analyze weak-types --ffi-evidence` 的每个 trait candidate 现在附带一段可直接粘贴的 `defexternal`
  骨架（`FfiTraitCandidate.defexternal_skeleton`）。
- 只纳入能表达为 Calcit trait 成员的字段/方法名，过滤 `aget`/`js-get` 产生的索引与字符串 key
  （如 `:0`、`:|b`）；没有可表达成员时骨架为空、不输出。
- 未知/未指定 target 不写 `:target`；字段与方法类型用 `Dynamic` 占位，`contract_status` 保持
  `review-required`，明确要求人工补全类型后再过严格质量门禁。
- 人类输出在有骨架时逐行缩进展示；JSON 由结构体序列化自动带出。
- 复用刚落地的 `defexternal`（#1223 P1/P2）：骨架经 `calcit edit def` 可解析、写盘 schema 归一化为
  `:: 'Trait`，`--check-only` 通过。
- 文档：`docs/features/js-interop.md` 补充骨架示例与 review-required 说明。
- 测试：`src/type_coverage.rs` 新增 `defexternal_skeleton_filters_unexpressible_members`。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全绿；
  docs check-md 69/69。
- 后续（仍属 #1224）：评估 review-only 的 `calcit fix` 建议形态，以及无法唯一确定时的保守边界。
