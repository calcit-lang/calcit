# 2026-0307-1652 — 解耦 snapshot::CodeEntry.schema 为 Arc<CalcitTypeAnnotation>

## 核心改动

将整个项目中 `snapshot::CodeEntry.schema` 和 `program::ProgramDefEntry.schema` 的类型，从 `Option<Edn>` / `Option<Arc<CalcitFnTypeAnnotation>>` 统一迁移为 `Arc<CalcitTypeAnnotation>`。

同时打破了 `type_annotation.rs → program → snapshot → calcit` 的循环依赖。

---

## 循环依赖的解法

**原依赖链（错误）：**

```
snapshot → calcit::type_annotation → program → snapshot  ❌
```

**修复方案：** 在 `type_annotation.rs` 中用 `OnceLock<fn>` 注册 program 级别的 lookup 函数，由 `program.rs` 在 `extract_program_data` 时调用 `register_program_lookups` 完成注册：

```rust
type LookupFn = fn(&str, &str) -> Option<Calcit>;
static LOOKUP_EVALED_DEF: OnceLock<LookupFn> = OnceLock::new();
static LOOKUP_DEF_CODE: OnceLock<LookupFn> = OnceLock::new();

pub fn register_program_lookups(evaled: LookupFn, code: LookupFn) {
  let _ = LOOKUP_EVALED_DEF.set(evaled);
  let _ = LOOKUP_DEF_CODE.set(code);
}
```

---

## 修改文件列表

### `src/calcit/type_annotation.rs`

- 移除 `use crate::program;` 导入
- 新增 OnceLock 静态注册机制及 `register_program_lookups` 公开函数
- 将 6 处 `program::lookup_*` 调用替换为本地 `lookup_evaled_def` / `lookup_def_code_registered`
- 新增 `CalcitTypeAnnotation::to_type_edn(&self) -> Edn` 序列化方法
- 新增 `calcit_type_to_edn(form: &Calcit) -> Edn` 私有辅助函数（Custom 类型回退）
- 新增 `CalcitFnTypeAnnotation::to_schema_edn(&self) -> Edn`（输出完整 schema map）
- 新增 `CalcitFnTypeAnnotation::to_schema_calcit(&self) -> Calcit`（用于 hint-fn 注入）
- 修复 `Self::Struct/Enum` 序列化：使用 `Edn::Tag` 而非 `Edn::Symbol`（类型不同）

### `src/calcit.rs`

- 在 `pub use type_annotation::` 中新增 `register_program_lookups`

### `src/program.rs`

- `ProgramDefEntry.schema: Arc<CalcitTypeAnnotation>`（取代旧 `Option<Arc<CalcitFnTypeAnnotation>>`）
- `extract_program_data` 首行调用 `calcit::register_program_lookups(...)`
- `lookup_def_schema` 返回类型改为 `Arc<CalcitTypeAnnotation>`（不再返回 Option，缺省返回 `DYNAMIC_TYPE`）
- `apply_code_changes` 中 `schema: DYNAMIC_TYPE.clone()` 取代 `None`

### `src/snapshot.rs`

- 移除 `CalcitFnTypeAnnotation` 从导入（已无直接使用）
- 新增 `mod schema_serde`：将 `Arc<CalcitTypeAnnotation>` 序列化为 `Option<Edn>` 保持二进制 RMP 兼容
- `CodeEntry.schema: Arc<CalcitTypeAnnotation>`，带 `#[serde(default, with = "schema_serde")]`
- `TryFrom<Edn>` / `From` trait 实现全部更新，使用 `parse_fn_schema_from_edn` + `DYNAMIC_TYPE`
- 测试用例中 `schema: Some(Edn)` 改为通过 `parse_fn_schema_from_edn` 构造 `Arc<CalcitTypeAnnotation>`

### `src/detailed_snapshot.rs`

- 同 `snapshot.rs` 模式：新增 `mod schema_serde`，字段类型改为 `Arc<CalcitTypeAnnotation>`
- `From<CodeEntry>` / `From<DetailedCodeEntry>` 均直接 clone Arc

### `src/runner/preprocess.rs`

- hint-fn 注入改为先匹配 `CalcitTypeAnnotation::Fn(fn_annot)`，再调用 `fn_annot.to_schema_calcit()`

### `src/bin/cli_handlers/edit.rs`

- schema 清空时：`schema = DYNAMIC_TYPE.clone()`
- schema 写入时：通过 `parse_fn_schema_from_edn` 解析后包装为 `Arc<CalcitTypeAnnotation::Fn(...)>`

### `src/bin/cli_handlers/query.rs`

- 4 处 `entry.schema.is_some()` / `if let Some(schema_edn)` 全部改为 `CalcitTypeAnnotation::Fn(fn_annot)` 匹配
- 序列化时通过 `fn_annot.to_schema_edn()` 转回 Edn 后再调用 `schema_edn_to_cirru`

### `src/bin/cr.rs`

- 新增 `CalcitTypeAnnotation` 到 `use calcit::{calcit::{...}}` 导入
- 原 `schema: None` 改为 `calcit::calcit::DYNAMIC_TYPE.clone()`
- `if let Some(schema_edn)` 改为 `if let CalcitTypeAnnotation::Fn(fn_annot)`

### `src/bin/cr_sync.rs`

- `schema: None` 改为 `calcit::calcit::DYNAMIC_TYPE.clone()`

---

## 关键设计决策

1. **`Arc<CalcitTypeAnnotation>` 永不为 None**：缺省值统一为 `DYNAMIC_TYPE`（全局 `LazyLock<Arc<CalcitTypeAnnotation>>` 单例），避免到处 `Option` 处理。

2. **二进制兼容**：`schema_serde` 模块确保 `Dynamic` → 序列化为 `None`，`Fn(...)` → 序列化为完整 Edn map，与旧 `Option<Edn>` 格式一一对应。

3. **`Edn::Tag` 用于类型名称**：`Struct`/`Enum` 类型序列化时用 `Edn::Tag(name)` 而非 `Edn::Symbol(name)`，因为 `EdnTag` 不能直接构造 `Edn::Symbol(Arc<str>)`。

---

## 验证结果

- `cargo build` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test` ✅（17 passed）
- `yarn check-all` ✅（515ms）
