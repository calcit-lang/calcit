# RFC: 类型自省(Type Introspection)一致性改进

状态：Implemented（1-3 项已落地，第 4 项按计划延后）
日期：2026-07-19

---

## 1. 概要

调研 Calcit 当前"查询一个类型能怎么交互"的能力——有哪些字段、有哪些方法(无论是内建还是通过 trait/impl 附加的)——发现相关能力**分散在多个不对齐的 API 里**，且存在几处"数据已经在运行时结构里，但没有暴露给 Calcit 代码"的具体 gap。本 RFC 记录调研结论，并给出可落地的修复项清单。

## 2. 调研结论

### 2.1 方法(methods)查询——统一性最好的部分

`&methods-of` / `&inspect-methods`([meta.rs](../src/builtins/meta.rs))把"内建方法"(list/map/string/number/set/fn 各自的 `&core-*-impls`)和"用户 `defimpl`/`impl-traits`"统一表示成 `CalcitImpl`，按优先级顺序去重合并，`&methods-of` 对内建/自定义一视同仁。

局限：`collect_impl_records_for_value` 只接受**实例值**(`Tuple`/`Record`/`List`/`Map`/`Number`/`Str`/`Set`/`Fn`/`Proc`)，不接受裸的 `Calcit::Struct`/`Calcit::Enum`/`Calcit::Trait`(类型定义本身)。也就是说 `&methods-of Person`(还没造实例)会直接报错，必须先构造一个实例才能查询"这个类型将来会有哪些方法"。

`NativeMethodsOf` 的类型签名是 `arg_types: vec![dynamic_tag()]`（[proc_name.rs](../src/calcit/proc_name.rs)），说明这一限制纯粹是**运行时实现**的限制，不是类型检查器强加的——修复它不需要碰类型系统。

### 2.2 字段(fields)查询——能用，但类型签名与实现脱节

`to-pairs`/`keys`/`&record:to-map` 在运行时(`to_pairs` in [maps.rs](../src/builtins/maps.rs))原生支持 `Calcit::Record`，能拿到字段名（tag）。

但 `ToPairs` 的类型签名声明 `arg_types: vec![some_tag("map")]`，而 `matches_with_bindings`([type_annotation.rs](../src/calcit/type_annotation.rs))里 `(TypeRef("map"), Record(base))` 这一分支是拿 `"map"` 去匹配 `base.name`（即结构体自己的名字，比如 `"Person"`），显然不相等 → **对 record 调用 `to-pairs`/`keys` 会在类型检查阶段被判定为类型不匹配并产生告警**，即便运行时完全正确。这是文档/类型签名与实际能力脱节的具体例子。

没有实例、只有裸类型定义时（刚 `defstruct Person ...` 还没构造实例），**没有**可编程 API 能拿到字段名列表当数据用；唯一能看到字段的方式是 `println`/`str` 对 struct 值做格式化输出人肉读。

### 2.3 `Display`（`println`/`str`）在三种类型定义值之间不对称

`impl fmt::Display for Calcit`（[calcit.rs](../src/calcit.rs)）：

| 类型定义值 | 输出示例 | 包含信息 |
| --- | --- | --- |
| `Struct` | `(%struct :Person (:name string) (:age number))` | 字段名 + 类型 |
| `Trait` | `(trait Name :m1 :m2)` | 仅方法名，**没有**方法签名类型 |
| `Enum` | `(%enum :Name)` | **仅名字**，连 variant 列表都没有 |

`CalcitEnum` 内部其实完整保存着 `variants()`（tag + payload_types，见 [sum_type.rs](../src/calcit/sum_type.rs)），数据都在，只是没接到 `Display` 里——这是一处"能力已具备却未暴露"的明显 gap，且没有任何测试覆盖这个输出格式（`grep "%struct"`/`"%enum"` 在 `calcit/*.cirru` 测试里零命中）。

### 2.4 结论

- 没有一个统一的"describe 类型"入口能同时列出 fields + methods（不管来源）；需要组合调用不同风格的 API。
- 对"类型定义本身"（还未实例化）的支持最弱：`&methods-of` 不接受、字段列表没有编程接口、`Enum`/`Trait` 的打印格式也比 `Struct` 缺信息。

## 3. 修复计划(按优先级/风险排序)

1. ✅ **`Enum` 的 `Display` 补上 variants**（最小、最安全，纯展示层修复，无 API 变化）。
   - 实现：[src/calcit.rs](../src/calcit.rs) `impl fmt::Display for Calcit` 的 `Enum` 分支，现在遍历 `enum_def.variants()` 输出 `(:tag type1 type2 ...)`。
   - 测试：`enum_display_includes_variants`（同文件 `#[cfg(test)] mod tests`）。
2. ✅ **`&methods-of` 接受裸 `Struct`/`Enum`/`Trait` 值**（直接读它们自带的 `impls` 字段），不再强制要求实例。
   - 实现：[src/builtins/meta.rs](../src/builtins/meta.rs) `collect_impl_records_for_value` 新增 `Struct`/`Enum` 分支；`iter_impls_in_precedence_order` 把 `Struct`/`Enum` 纳入与 `Tuple`/`Record` 相同的“后定义优先”倒序规则；`Trait` 单独处理（直接读 `trait_def.methods`，因为 trait 本身没有 `impls` 字段，是直接声明方法名）。`methods_of`/`inspect_methods` 都做了对应分支。
   - JS 同步：[ts-src/calcit.procs.mts](../ts-src/calcit.procs.mts) 的 `lookup_impls`/`_$n_methods_of`/`_$n_inspect_methods` 同步补齐 `CalcitStruct`/`CalcitEnum`/`CalcitTrait` 分支。
   - 测试：[calcit/test-traits.cirru](../calcit/test-traits.cirru) 的 `test-method-introspection` 新增对 `impl-traits Person0 MyFooImpl`（裸 struct）、`DemoBar`（裸 enum）、`MyFoo`（裸 trait）调用 `&methods-of` 的断言，`yarn check-all`（rs/js/ir/wasm 四目标）全部通过。
3. ✅ **修正 `to-pairs`/`keys` 的类型签名**，让 record 也能匹配，消除虚假类型告警。
   - 实现：[src/calcit/type_annotation.rs](../src/calcit/type_annotation.rs) `matches_with_bindings` 里 `(TypeRef, Record)` 分支新增：当 `TypeRef` 名字就是泛化的 `"map"` 时，结构性地匹配任意 record（不要求 record 名字等于 `"map"`）。
   - 测试：新增单元测试 `generic_map_type_ref_accepts_records_structurally`，直接验证 `TypeRef("map")` 与 `Record(Person)` 现在双向匹配，同时确认无关的 `TypeRef` 名字仍然不会误匹配。
   - 备注：实测 `calcit --check-only` 在当前仓库测试集里并未因这个 gap 产生可观察的告警（`test-record.cirru` 里 `keys p2` 这行本来就没有触发过告警，猜测是这条路径上的静态类型推断没有把 `p2` 识别为具体的 `Record` 类型，所以没有触发 `check_proc_arg_types` 这条检查分支）。但 `matches_with_bindings` 的错误比较逻辑本身是真实存在的 bug，属于防御性修复：一旦未来静态推断能力增强（例如给变量加显式类型标注后传入 `to-pairs`/`keys`），就不会再误报。
4. ⏸️ **（可选，视时间，本轮未实现）** 新增 `&struct:fields`/`&enum:variants` 之类的编程接口，让“裸类型定义”的字段/variant 列表也能被程序消费，而不仅仅是打印文本。
   - 未实现原因：新增一个 proc 需要贯穿 `proc_name.rs`(注册+类型签名)、`builtins.rs`(分发)、`builtins/records.rs`或`meta.rs`(实现)、以及 JS/IR/WASM 三个 codegen 目标的同步实现，工作量与收益相比前三项更低（前三项已经解决了运行时能力不对齐的核心问题；字段/variant 名字目前仍可通过 `Display`/`println` 人肉获取，只是没有编程接口）。留作后续独立迭代。

## 4. 兼容性 / 风险

- 第 1、2、3 项都已实现并通过 `cargo test`、`cargo run --bin calcit -- calcit/test.cirru`、`yarn check-all`、`cargo clippy -- -D warnings`、`cargo fmt` 验证，纯粹的能力扩展/告警范围放宽，不改变现有行为，向后兼容。
- 第 4 项延后，不影响现有行为。

## 5. 验证方式（已执行）

- `cargo test`：新增/更新的 Rust 单元测试全部通过，覆盖 `Display for Enum`、`&methods-of` 对裸类型定义的调用、`to-pairs`/`keys` 的类型匹配放宽。
- `cargo run --bin calcit -- calcit/test.cirru`：Cirru 集成测试套件全部通过，包括新增的裸类型 `&methods-of` 断言。
- `yarn check-all`：JS/IR/WASM 三个目标同步验证通过（`&methods-of` 的 JS 实现已同步补齐）。
- `cargo clippy -- -D warnings` / `cargo fmt`：均无告警。
