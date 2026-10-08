# 退场 Map/Set `.contains?` 与 Map `.includes?`

#1826 删除了一批集合方法别名，但留下了 Map/Set `.contains?` 与 Map `.includes?`：Map 与 Set 还通过 `Contains` trait 提供 `.contains?`，`src/calcit/type_annotation.rs` 的 `builtin_core_trait_names` 也为它们列出 `Contains`，当时该文件有并行改动。本次补上这一组，0.29.0 一起删除。

删除项：

- Map `.contains?`（首选 `.contains-key?`）、Map `.includes?`（首选 `.contains-value?`）
- Set `.contains?`（首选 `.includes?`，同一实现）
- Map 与 Set 的 `Contains` trait impl（`&core-contains-map-impl`、`&core-contains-set-impl`）
- fix 规则 `core-predicate-method-v1`：List/String 部分已在更早退役，剩下的三项随旧方法消失，整条规则退役

可以删除 trait impl 的依据：

- 下游：respo.calcit、calcit.std、corokia、calcit-graphviz、cumulo-util.calcit、memof、js-ffi、calcit-paint、calcium-workflow 的默认分支没有 Map/Set `.contains?`、Map `.includes?` 方法调用，也没有任何 `Contains` 引用（含 `:where` 约束）。calcit.std 的 `.includes?` 全部作用于 List/String。
- 仓库内依赖 Map/Set `Contains` 的只有两处：`test-traits` 的 `contains-with-trait?` 用 Map/Set 调用了 `'T 'Contains` 约束，已改为只用 Struct/Enum；前缀函数 `contains?` / `includes?` 对类型未知的接收者回落到 `.contains?` / `.includes?` 方法。下游大量使用前缀 `contains?`（respo 18 处、corokia 3 处、memof 4 处），所以两个函数改为先判断 `map?` / `set?` 再调用 `&map:contains?` / `&set:includes?` / `&map:includes?`，语义与 0.28 相同，新增的 `:tests` 覆盖异质 List 中的开放接收者。
- 类型已知的前缀调用原本就在预处理阶段 lowering 到 `NativeMapContains` / `NativeSetIncludes`，不经过 trait。
- List 与 String 此前已从 `Contains` 移出，Map/Set 按同一做法处理；`Contains` 现在只由 Struct 与 Enum 实现。

保留项：

- Struct `.contains?`（`.contains-field?`）与 Enum `.contains?`（`.contains-index?`）语义不完全等价，没有 fix 规则，按计划留到 0.30.0，与 `Contains` trait 的去留一起处理（#1482）。
- WASM `emit_wasm/methods.rs` 中通用的 `.contains?` / `.includes?` 方法分支仍服务 Struct/Enum 与 List/Set/String，不删除。

实现：

- core 方法表删去三行，`&core-map-impls` / `&core-set-impls` 不再挂 `Contains`；`builtin_core_trait_names` 同步。
- `retired_method_migration` 为三个方法给出 `E_RETIRED_METHOD` 与首选写法，提示中说明前缀 `contains?` 仍可用。
- fix：`core-predicate-method-v1` 加入 `RETIRED_CORE_API_028_RULES`，从两个 preset 删除；`core-api-0.28-v1` 由 7 条减为 6 条，`core-api-0.29-v1` 由 8 条减为 7 条。`QUERYABLE_METHOD_ALIASES` 只剩 FfiTask 效果别名，因此直接使用 `CORE_EFFECT_METHOD_ALIASES`；`MethodReceiverKind` 删去 Map/Set 变体，`collapse_predicate_alias_suggestions` 及其测试一并删除。

验证：`strict_type_fail_retired_alias_methods_report_preferred_spelling` 覆盖 3 个新退役方法；`retired_core_method_alias_rules_point_to_the_0_28_cli` 覆盖 `core-predicate-method-v1`；`bootstrap_core_trait_names_match_core_impl_definitions`；`check-agent-interface` 断言 `query type` 不再列出这些方法；`calcit.core/contains?` 与 `calcit.core/includes?` 的 `:tests`；`cargo test`、`yarn check-all`。
