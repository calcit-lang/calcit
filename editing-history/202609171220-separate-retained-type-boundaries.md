# 区分迁移债务与保留的类型边界

- 项目级严格迁移 manifest 只把 unresolved、declared Optional 和 explicit unsafe 归入 review-required。
- 已明确声明的 JS FFI 与 type-slot Dynamic 单独列为 retained boundaries。
- macro syntax 与明确的 Unit 返回不进入迁移债务，避免工作流重新制造统计型噪声。
