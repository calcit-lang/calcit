# 2026-0307 移除 CalcitEnum 的 prototype 字段

## 背景

老版本的 `CalcitEnum` 使用 `prototype: Arc<CalcitRecord>` 存储枚举定义（字段=variant tag，值=payload 类型列表），因为当时 enum 借助 record 的数据结构来定义。现在 enum 已有独立的定义语法（`defenum`），prototype 字段冗余，予以移除。

## 主要变更

### `src/calcit/sum_type.rs`

- `CalcitEnum` 结构体：`prototype: Arc<CalcitRecord>` → `name: EdnTag`
- `from_record` / `from_arc`：提取 name，不再存储整个 record
- 删除 `prototype()` 方法，新增 `name()` 返回 `&EdnTag`
- 新增 `to_record_prototype() -> CalcitRecord`：根据 `name + variants` 按需重建 `CalcitRecord`，供 `preprocess.rs` 向后兼容使用

### `src/calcit.rs`

- `Hash` 实现：改为对 variant tag + payload type 逐项哈希
- `PartialEq` 实现：`a.prototype() == b.prototype()` → `a.name() == b.name() && a.variants() == b.variants()`

### `src/calcit/compare.rs`

- `compare_calcit_enum_values`：不再对比 prototype record，改为先比较 name，再逐 variant 比较 tag 和 payload types（深度比较），确保 `Ord::cmp == Equal` ⟺ `PartialEq::eq == true`

### `src/data.rs`

- `Enum(enum_def)` 序列化分支：通过 `enum_def.name()` 和 `enum_def.variants()` 直接读取，不再依赖 prototype record

### `src/runner/preprocess.rs`

- `resolve_record_value` 中所有 `Calcit::Enum(enum_def)` 分支：`enum_def.prototype().to_owned()` → `enum_def.to_record_prototype()`

## 验证结果

- `cargo build` ✅（无警告）
- `cargo test` ✅（94 + 17 = 111 通过，0 失败）
- `cargo clippy -- -D warnings` ✅（无警告）
- `yarn check-all` ✅（exit 0）

## 踩坑与注意

- `compare_calcit_enum_values` 最初只比 tag 未比 payload types，导致 `cmp_equal_matches_eq_for_complex_named_variants` 失败。修复时需对每个 variant 的 payload types 也做深度比较。
- `to_record_prototype()` 是按需重建而非缓存，性能上无问题（仅在类型检查预处理阶段调用）。
