# Component Struct record adapter

## 中文

- 在 Component ABI type 中加入 monomorphic Struct record，按规范化 schema field 顺序递归生成 flat shape 与 memory layout。
- 新增 Struct lift/lower codec，验证 canonical memory、内部 field count 与 nominal tag，并复用现有 Bool、String、List、Option/Result 和嵌套 Struct walker。
- 在共用 schema 阶段强制 Canonical ABI 的 16 个 flat parameter 上限，import/export 对超宽 Struct 使用同一稳定诊断。
- 从编译后的 nominal declaration 解析 Struct schema，并把仅出现在类型边界的 Struct/field tag 纳入确定性 WASM tag index。
- 在 Calcit definition `:tests` 与 JS/WASM integration 中覆盖嵌套 Struct 的 export/import 往返和非法 Bool 拒绝。
- 更新 Component boundary 与升级文档，保持 `calcit ffi export`、`calcit wasm` 和 bindgen `generate/check` 的入口边界。

## English

- Added monomorphic Struct records to Component ABI types, recursively deriving flat shapes and memory layouts in normalized schema-field order.
- Added Struct lift/lower codecs that validate canonical memory, internal field counts, and nominal tags while reusing the existing Bool, String, List, Option/Result, and nested-Struct walker.
- Enforced the Canonical ABI 16-flat-parameter limit in the shared schema stage so imports and exports reject wide Structs with the same stable diagnostic.
- Resolved Struct schemas from compiled nominal declarations and included Struct/field tags that appear only at typed boundaries in the deterministic WASM tag index.
- Covered nested Struct export/import round trips and invalid Bool rejection in Calcit definition `:tests` and JS/WASM integration.
- Updated Component-boundary and upgrade documentation while keeping the existing `calcit ffi export`, `calcit wasm`, and bindgen `generate/check` ownership split.
