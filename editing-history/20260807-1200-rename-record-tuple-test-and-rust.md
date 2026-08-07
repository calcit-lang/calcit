# Struct / Enum 命名全量对齐（测试夹具 + Rust 模块）

延续 data model v2，把代码库中遗留的 `record`/`tuple` 命名统一迁移到 `struct`/`enum`，分两层完成：

## A 级：测试夹具文件与引用

- `calcit/test-record.cirru` → `calcit/test-struct.cirru`（namespace `test-record.main` → `test-struct.main`，package/FileEntry/init-fn/reload-fn/schema args/ns 声明全部同步；内部 def 同步改名：`test-record-with`→`test-struct-with`、`test-partial-record`→`test-partial-struct`、`test-loose-record-rewrite`→`test-loose-struct-rewrite`、`test-map-to-record`→`test-map-to-struct`）。
- `calcit/test-tuple.cirru` → `calcit/test-anonymous-enum.cirru`（namespace `test-tuple.main` → `test-anonymous-enum.main`；因已有 `test-enum.cirru`，采用与文档 `anonymous-enums.md` 平行的命名；该文件测试 anonymous enum + destruct 操作）。
- 更新全部引用：`calcit/test.cirru`（模块列表/requires/入口/CodeEntry key）、`calcit/test-wasm-suite.cirru`、`src/bin/cli_handlers/query.rs`（`test-record.main`→`test-struct.main`）、`scripts/check-agent-interface.mjs`、`scripts/test-wasm-suite.sh`、`scripts/test-wasm-suite-extended.sh`、`docs/run/query.md`。
- 其他测试文件中的内部 def 名同步：`test-record-methods`→`test-struct-methods`（test-types.cirru + `.calcit/cursor.cirru`）、`test-tuple-impl-precedence-order`→`test-enum-impl-precedence-order`（test-traits.cirru）、`test-tuple-to-enum`→`test-anonymous-enum-to-named`（test-enum.cirru）、`test-record-inference`→`test-struct-inference`（test-types-inference.cirru）。
- **保留**：`test-wasm.cirru` / `scripts/test-wasm.mjs` 未动——该文件是 legacy 兼容测试（仍用 `defrecord`、`tuple-enum`、`:tuple` tag），内部 `test-record-*`/`test-tuple-*` 名称与其 legacy 语义一致。

## B 级：Rust 内部模块与函数命名

- 模块文件重命名 + mod 声明 + 路径引用：
  - `src/calcit/record.rs` → `src/calcit/struct_value.rs`（`mod record`→`mod struct_value`；`record::` → `struct_value::`）
  - `src/calcit/tuple.rs` → `src/calcit/enum_value.rs`（`mod tuple`→`mod enum_value`）
  - `src/builtins/records.rs` → `src/builtins/structs.rs`（`mod records`→`mod structs`；`records::` → `structs::`，含 builtins.rs 分发与 type_inference.rs）
  - `src/codegen/emit_wasm/records.rs` → `src/codegen/emit_wasm/structs.rs`（含 `#[path = "emit_wasm/structs.rs"]` 修正）
  - `src/calcit/sum_type.rs` 保留（"sum type" 是 enum 的合法名称，无 legacy 措辞）。
- 模块内 legacy 函数名迁移（含 call sites）：`call_record*`→`call_struct*`、`record_with`→`struct_with`、`record_nth`→`struct_nth`、`record_field_tag`→`struct_field_tag`、`record_assoc_at`→`struct_assoc_at`、`record_with_at`→`struct_with_at`、`record_from_map`→`struct_from_map`、`get_record_name`→`get_struct_name`、`get_record_struct`→`get_struct_def`；WASM emitter：`emit_record*`→`emit_struct*`、`emit_tuple*`→`emit_enum*`、`emit_enum_tuple_new`→`emit_named_enum_new`、`emit_tuple_*_from_local`→`emit_enum_*_from_local`。

## 有意保留（B 级深层内部，附理由）

- `NativeRecord*` / `NativeTuple*` 内部分发枚举变体：存在命名冲突（`NativeRecordImplTraits` 与已存在的 `NativeStructImplTraits` 并存且分发不同函数），且部分新名已存在，盲改不安全；为兼容性代码库有意保留新旧两套。
- 局部变量 `record`/`tuple`（约 700+）：大量出现在外部 `Edn::Record`/`Edn::Tuple`（cirru_edn crate）上下文——那是 EDN 自己的 Record/Tuple 语义，改名为 struct/enum 反而错误；其余为内部局部绑定，纯外观、零用户价值、改动面大。
- 注释与诊断字符串中的措辞、RFC/editing-history 历史记录。

## 验证

- `cargo test` 全绿（360 + 2 + 180，0 失败）；`cargo clippy --lib -- -D warnings` 通过。
- `cargo clippy --all-targets -- -D warnings` 在 `src/bin/calcit_deps.rs` 报 `items after a test module`——该文件未改动，属分支既有问题。
- `check-agent-interface` 12 项场景全过（`test-struct.main` 解析正常）；WASM codegen 与 `test-wasm-suite` 通过。
- 已知既有问题（与本次无关）：`try-rs`（`cr calcit/test.cirru`）在 `test-generics` 栈溢出，clean worktree 亦复现。

## 后续安全术语迁移

- `src/calcit/data_patch.rs`、`src/calcit/data_shape.rs`：将 `Calcit::Struct(record)` / `Calcit::Enum(tuple)` 的内部绑定改为 `struct_value` / `enum_value`，并将仅面向当前数据模型的诊断与测试断言改为 struct/enum。
- `src/call_tree.rs`：枚举 payload 递归遍历绑定改为 `enum_value`。
- `src/data/edn_decode.rs`：EDN 解码测试中的 Calcit struct/enum 解构绑定和断言改为新术语；`CalcitEnumDef::from_record` 保持不动，因为它明确承载旧 enum-definition schema 的兼容转换。
- 以上四个切片分别通过 `data_patch::tests`（4/4）、`data_shape::tests`（6/6）、`call_tree`（1/1）和 `edn_decode::tests`（7/7）。文档检索后仅余迁移表与兼容说明，未发现需改写的当前模型表述。
