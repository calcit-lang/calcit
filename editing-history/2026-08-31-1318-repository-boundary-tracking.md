# Repository boundary tracking

## 中文

- 记录 Calcit 主仓库与独立产品的当前边界：语言语义、backend、CLI/Agent 和权威文档留在 core。
- `calcit-bindgen` 与 `calcit-native-ffi` 已独立；`caps` 和 Calx benchmark harness 进入拆分规划。
- 短期不规划 LSP，因此不为假设中的 consumer 拆出 analysis 仓库。
- 固化拆分模块文档规则：README/AGENTS 必须说明状态、职责、source of truth、兼容/发布策略、迁移验证与 Issues。

## English

- Documented the boundary between Calcit core and independently maintained products.
- Recorded the existing bindgen/native-ffi repositories and the planned caps/Calx benchmark extractions.
- Explicitly kept analysis and Agent CLI capabilities in core because no near-term LSP is planned.
- Added a durable README/AGENTS documentation contract for extracted, shared, template, and experimental modules.
