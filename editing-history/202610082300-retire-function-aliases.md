# 退场函数 `some?`、`join-str`、`add-watch`、`cpu-time` 与 String `.count`

#1826 和 #1834 删除了集合方法别名，函数别名与 String `.count` 当时还有下游调用。corokia#40（`some?`、`add-watch`、`cpu-time`）、calcit-graphviz#32（`join-str`）、calcit.std#84（String `.count`）合并后，这一组在 0.29.0 删除。

删除项：

- 函数 `some?`（首选 `non-nil?`）、`join-str`（首选 `join-string`）、`add-watch`（首选 `add-watch!`）、`cpu-time`（首选 `monotonic-time-ms`）
- String `.count` 方法（首选 `.len`）
- fix 规则 `core-non-nil-predicate-v1`：只迁移 `some?`，整条退役，加入 `RETIRED_CORE_API_028_RULES`

下游核对：respo.calcit、calcit.std、corokia、calcit-graphviz、cumulo-util.calcit、memof、js-ffi、calcit-paint、calcium-workflow 与 recollect、reel.calcit、respo-ui.calcit 的默认分支，`*.cirru` 中没有这四个函数名，也没有 String 接收者的 `.count`。JS 测试里的 `c.count(...)` 调用的是 `count` 函数。文档仍有旧名示例：respo.calcit 的 README 与 docs（`add-watch`、`some?`）、calcium-workflow 的 Agents.md（`some?`）、corokia 的 README（`join-str`），需要在下游单独更新。

实现反转（首选名先接管实现，再删旧名）：

- `join-string` 直接包含原 `join-str` 的实现；List 方法表 `:join-string` 指向 `join-string`；WASM 的调用点拦截与导出构建从 `join-str` 改为 `join-string`。
- proc `cpu-time` 改名为 `monotonic-time-ms`（`CalcitProc::MonotonicTimeMs`），core 定义改为 `&runtime-implementation`；JS 导出 `cpu_time` 改为 `monotonic_time_ms`；WASI lowering、effects graph 与 macro capability 同步。宏 `with-cpu-time` 保留名字，展开改为调用 `monotonic-time-ms`。
- proc `add-watch` 改名为 `add-watch!`，core 定义改为 `&runtime-implementation`，类型签名不变；JS 导出改为 `add_watch_$x_`，错误信息改用新名。`remove-watch!` 仍包装 `remove-watch`。
- `non-nil?` 本来就自带实现，不需要反转。

String `.count`：String 仍实现 `Countable`，`count` 函数、`&trait-call Countable :count` 与 `where T: Countable` 的泛型调用都要通过这个 impl。因此只删除 `&core-string-methods` 中的 `:count`，并由 `retired_trait_reachable_method_migration` 在具体 String 接收者上先于 impl 解析报告 `E_RETIRED_METHOD`；`static_method_descriptors` 也不再列出它。List/Map/Set/Struct/Enum 的 `.count` 不变。

诊断：

- 函数沿用 `foldl'` 的方式，从 core 删除后按未知名字报告；`retired_core_function_migration` 在同一条告警末尾补上首选写法（含 `foldl'`），不新增诊断编号。
- `retired_method_migration` 为 String `.count` 给出 `E_RETIRED_METHOD`。

fix 规则：

- `core-non-nil-predicate-v1` 退役，`core-api-0.28-v1` 由 6 条减为 5 条，`core-api-0.29-v1` 由 7 条减为 6 条。
- `core-function-alias-v1` 仍迁移 `optionally`、`join`、`vals`，去掉 `join-str`。
- `core-collection-len-v1` 仍迁移 List/Map/Set 的 `.count`，去掉 String。

保留项：`vals`、`turn-string`、`turn-str`、`remove-watch`、`optionally`、`join`、`%ok`/`%err`、`turn-symbol`、`atom`/`defatom` 仍有下游使用或未满足退场条件。

验证：`calcit.core/count#string-keeps-countable-after-method-retirement`、`join-string`、`add-watch!`、`monotonic-time-ms` 的 `:tests`；Rust `retired_core_functions_report_preferred_spelling` 与 `strict_type_fail_retired_alias_methods_report_preferred_spelling`（String `.count`）；`retired_core_alias_rules_point_to_the_0_28_cli` 覆盖 `core-non-nil-predicate-v1`；`check-agent-interface` 断言 `query type 'String` 不再列出 `.count`；`cargo test`、`yarn check-all`。
