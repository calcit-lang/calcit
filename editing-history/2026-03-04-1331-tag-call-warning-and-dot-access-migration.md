# trait/impl 方法键 tag 写法 warning 与点号迁移评估

## 背景

- `deftrait` / `defimpl` 里方法键目前使用 `:foo` 形式，语义上对应方法名。
- 为逐步迁移到点号方法表示（如 `.foo`）需要先在 trait/impl 场景给出提示，避免误伤普通 tag/map 使用。

## 本次改动

- 在 `src/runner/preprocess.rs` 的宏预处理分支新增 warning：
  - 仅在 `deftrait` / `defimpl` 检查方法键（默认展示）。
  - 检测到 `:foo` 方法键时提示迁移到 `.foo` 风格（兼容性保留）。
- 移除对普通 `(:k obj)` 访问调用的迁移 warning，避免范围跑偏。
- 新增单测 `warns_on_trait_impl_method_tag_syntax`，确保 warning 范围和文案稳定。
- 同步在 `calcit/*.cirru` 的 trait/impl 测试与示例中开始迁移到 `.foo` 写法。

## 迁移建议（分阶段）

1. **当前阶段（已落地）**：默认 warning + 兼容旧写法。
2. **下一阶段**：在文档与脚手架模板中默认采用 `.foo` 方法键表示。
3. **后续阶段（可选）**：提供自动改写工具（`deftrait/defimpl` 的 `:foo` -> `.foo`）并支持批量修复。
4. **最终阶段（可选）**：再评估是否提升 warning 严格度（例如 lint 级别开关）。

## 验证

- 定向测试：
  - `cargo test warns_on_trait_impl_method_tag_syntax -- --nocapture`
  - `cargo test warns_on_dynamic_trait_call -- --nocapture`
