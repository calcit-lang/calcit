# Record / Tuple 命名收尾清理（data model v2 术语全量对齐）

延续 data model v2 与 `20260807-1200-rename-record-tuple-test-and-rust.md` 的「后续安全术语迁移」计划，
把代码库中剩余的 `record`/`tuple` 措辞在**安全**的前提下统一迁移到 `struct`/`enum`。本次改动仅涉及 Rust 源码（32 个文件），文档无需再改（上次已同步）。

## 迁移内容

### 内部函数重命名（含调用点）

- `compare_record_values` → `compare_struct_values`（`src/calcit/compare.rs` + `calcit.rs`）
- `CalcitEnumDef::to_record_prototype` → `to_struct_prototype`（`sum_type.rs` + type_inference / type_rewriting 调用点）
- `infer_record_field_type/get_type/nth_type/literal_type` → `infer_struct_*`；`infer_struct_literal_type` 与新模型 `%{}` 冲突，新值字面量推断改名为 `infer_struct_value_literal_type`
- `check_record_field_access/update_fields/method_args` → `check_struct_*`；`check_field_in_record` → `check_field_in_struct`
- `check_tuple_nth_bounds` → `check_enum_nth_bounds`；`check_enum_tuple_construction` → `check_enum_construction`（`mod.rs` + `type_checking.rs` 导入与测试调用点）
- `resolve_record_value` → `resolve_struct_value`；`resolve_record_def` → `resolve_struct_value`（type_annotation.rs，避免与既有 `resolve_struct_def` 冲突）
- `collect_record_literal_values` → `collect_struct_literal_values`
- `is_tuple_constructor` → `is_enum_constructor`（type_inference + type_annotation）
- `infer_enum_tuple_annotation` → `infer_enum_annotation`；`infer_enum_tuple_applied_args` → `infer_enum_applied_args`
- type_rewriting.rs：`try_rewrite_map_args_to_records` → `try_rewrite_map_args_to_structs`、`try_rewrite_single_map_to_record` → `try_rewrite_single_map_to_struct`、`try_rewrite_loose_record_args_to_struct_records` → `try_rewrite_loose_struct_args_to_structs`、`try_rewrite_single_loose_record_to_struct_record` → `try_rewrite_single_loose_struct_to_struct`、`try_rewrite_tuple_args_to_enum_tuples` → `try_rewrite_enum_args_to_named_enums`、`try_rewrite_single_tuple_to_enum_tuple` → `try_rewrite_single_enum_to_named_enum`、`try_rewrite_local_fn_tuple_args_to_enum_tuples` → `try_rewrite_local_fn_enum_args_to_named_enums`
- builtins/meta.rs：`new_tuple` → `new_enum_value`、`new_enum_tuple_no_class` → `new_named_enum_value`、`tuple_enum` → `enum_definition`、`tuple_enum_has_variant` → `enum_def_has_variant`、`tuple_enum_variant_arity` → `enum_def_variant_arity`、`tuple_validate_enum` → `enum_validate`、`tuple_nth` → `enum_nth`、`tuple_count` → `enum_count`、`tuple_impls` → `enum_impls`、`tuple_params` → `enum_params`、`parse_enum_record` → `parse_enum_struct`（builtins.rs 分发同步）
- builtins/structs.rs：`checked_record_index` → `checked_struct_index`
- gen_ir.rs：`dump_record_code` → `dump_struct_value_code`、`record_metadata` → `struct_value_metadata`、`dump_tuple_code` → `dump_enum_value_code`、`tuple_metadata_entries` → `enum_value_metadata_entries`（与既有 `dump_struct_code`/`dump_enum_code` 错开）
- effects_graph.rs：`find_best_record_template` → `find_best_struct_template`、`walk_for_record_templates` → `walk_for_struct_templates`、`format_record_fields` → `format_struct_template_fields`（与既有 `format_struct_fields` 错开）、`parse_record_field_pairs` → `parse_struct_field_pairs`、`parse_flat_record_pairs` → `parse_flat_struct_pairs`、`parse_nested_record_entries` → `parse_nested_struct_entries`、`record_fields_from_calcit` → `struct_fields_from_calcit`、`is_record_literal_head` → `is_struct_literal_head`
- type_coverage.rs：`read_schema_param_tuple` → `read_schema_param_wrapped`
- WASM codegen：`emit_option_tuple` → `emit_option_enum`、局部变量 `tuple_tag` → `enum_tag`、`record_tag` → `struct_tag`、`tuple_ptr` → `enum_ptr`、`try_format_tuple_literal` → `try_format_enum_literal`
- `CalcitImpl::from_record` → `CalcitImpl::from_struct`（calcit_impl.rs + type_annotation.rs 调用点）；`as_record` 冗余别名删除（`as_struct` 已存在）；`as_tuple` → `as_enum`
- `resolve_enum_value`/`resolve_enum_from_program`/`resolve_enum_def` 内 `Calcit::Struct(record)` 绑定 → `struct_value`

### 局部绑定与测试夹具

- 模式绑定 `Calcit::Struct(record)` / `Calcit::Enum(tuple)` → `struct_value` / `enum_value`（全仓库，约 40 处）
- 测试夹具变量：`enum_record` → `enum_struct`、`test_record` → `test_struct`、`class_record` → `class_struct`、`indexed_record_fixture` → `indexed_struct_fixture`、`sample_enum_record` → `sample_enum_struct`、`enum_left/right_record` → `enum_left/right_struct`
- 测试名（可见于测试输出）：`cmp_records_*` → `cmp_structs_*`、`*record_field*` → `*struct_field*`、`checks_enum_tuple_*` → `checks_enum_*`、`checks_tuple_nth_*` → `checks_enum_nth_*`、`schema_rest_named_tuple_is_treated_as_type_only` → `schema_rest_named_enum_*`、`test_normalize_schema_unwraps_wrapped_*_tuple` → `*_enum` 等

### 注释 / 诊断字符串

- `mod.rs`、`builtins/*`、`codegen/emit_wasm/*`、`data/cirru.rs`、`calcit/syntax_name.rs` 等 60+ 处注释与 doc 措辞同步
- 用户可见错误/警告串：`[Warn] record update field` → `struct update field`、`map-to-record` → `map-to-struct`、`loose-record-to-struct` → `loose-struct-to-struct`、`tuple-to-enum` → `enum-to-named-enum`、`record {} does not define field` → `struct`、`&list:foldl-shortcut expected a value in the tuple` → `in the enum`

## 有意保留（附理由）

- `NativeRecord*` / `NativeTuple*` 内部分发枚举变体：与既有 `NativeStruct*` / `NativeEnum*` 存在真实命名冲突（如 `NativeRecordImplTraits` = `&struct:impl-traits` 与 `NativeStructImplTraits` = `&struct-def:impl-traits` 并存且分发不同），盲改不安全。配套的 `record_impl_traits` / `tuple_impl_traits` 函数名同步保留。
- `CalcitEnumDef::from_record`：明确承载旧 enum-definition schema 的兼容转换，保留。
- `Edn::Record` / `Edn::Tuple`、`EdnRecordView` / `EdnTupleView`、`Edn::enum_tuple`、`DefinitionRecord`、`find_edn_record_value_mut`、`find_record_in_options`、`legacy_snapshot_record_summary`、`empty_record`（快照测试）：外部 `cirru_edn` crate 的 EDN 语义，改名反而错误。
- 旧输入拼写兼容：`type_annotation.rs` 中 `"record"/"Record"/"tuple"/"Tuple"` 的解析匹配、`:: :record`/`:: :tuple` 兼容分支、`custom_keyword_matches(..., "record"/"tuple")` 运行时类型校验——保留以兼容旧写法。
- legacy `defrecord` 语法兼容：`try_parse_defrecord_form` / `is_defrecord`（WASM codegen 解析旧 `defrecord` 表单）。
- impl records（trait 概念）：`collect_*_impl_records*`、`get_impl_records_from_type`、`resolve_core_impl_records`、`method_record`、`imp_record`、`impl_record` 等——与文档中保留的「impl records」术语一致。
- Hash 鉴别串：`"record:"` / `"tuple:"`（`Calcit::Hash`）、`"record"` / `"dyntuple"`（type annotation Hash）——`&hash` 内建会把结果暴露给用户，改动会改变 hash 输出，属于行为变更，不纳入术语清理。
- 遗留行为测试名：`nominal_options_warn_on_legacy_nil_and_tuple_operations`（测试的是对旧 tuple 操作的告警）。
- 文档中的合法旧名引用：upgrade 迁移表、`impl records`、`namespace records`、动词 `records`、旧 `tuple` where 语法告警。
- `src/cirru/calcit-core.cirru` 中 `record-match` 已标记为 removed legacy macro，保留。

## 验证

- `cargo fmt` 通过；`cargo clippy --lib -- -D warnings` 通过。
- `cargo test`：360 + 2 + 184 = 546 用例全绿（修复 1 处 `validates_typed_struct_update_fields_after_generic_substitution` 断言串仍引用旧 `record update field`）。
- `cargo clippy --all-targets` 仅剩 `src/bin`（caps/calcit_deps）既有 `items after a test module` 警告，未改动文件，与本次无关。
- `yarn compile`、`yarn check-js-runtime`、`yarn check-agent-interface`（12/12）、`yarn try-rs`、`yarn try-js`、`yarn try-ir`、`yarn try-wasm` 全部通过。

## 备注

- 修改类编辑统一用 `search-replace`/上下文定位，避免索引漂移；遇到多副本局部变量（如 WASM `tuple_tag`/`tuple_ptr`、测试夹具 `enum_record`）用语言服务器语义重命名（rename symbol）按函数作用域逐一处理。
