# 退场 List/Map/Set 方法别名与 foldl'

0.29.0 是非 patch 版本。#1458 第二轮盘点（12 个活跃下游仓库的默认分支）列出一批零用量的兼容入口，本次在 core 当前 main 上逐项复核后，只删除仍满足三道闸门的条目：已知活跃下游没有调用、首选入口与 fix 规则已在正式版发布、实现可以干净解耦。

删除项：

- List `.reduce`、`.bind`、`.join-str`、`.nth`（首选 `.fold`、`.flat-map`、`.join-string`、`.get`，0.27.0 发布）
- Map `.mappend`（`.merge`）、Map `.add`（`.assoc key value`，旧方法只校验 entry 长度，没有 fix 规则）
- Set `.add`（`.include`）、`.mappend`（`.union`）
- 函数 `foldl'`（`fold`，参数顺序相同；同时删除 WASM 中 3 处特判）

复核时额外浅克隆了 #1819 记录的阻塞仓库（diary、timegrass、edn-renderer、hovenia-editor、calcit-error-viewer、corokia、calcit-fetch、calcit-wss、stir-template、toadflax、msg-buffer、lutea-reader、cirru.org、lilac、editor 等），发现以下候选仍有真实调用，因此留在 0.30.0：`optionally`、`%ok` / `%err`、`turn-str`、`remove-watch`、`cpu-time`（corokia 宏模板）、函数 `round?`、`turn-symbol`、`join`、List/Map `.count`（timegrass 测试）、FfiTask `.cancel-with`（calcit-fetch）及 `.cancel`（多个模块文档）。List `.add` 因 List 同时实现 `Add` trait（`.add` = `&list:concat`），删掉方法表一行会让旧调用静默改走拼接，需先决定 List 的 `Add` 实现。`result:ok?` / `result:err?` / `result:unwrap-or` 是 `ResultOpsImpl` 的方法实现目标，与 `option:*` 一起在 0.30.0 处理；`tuple?` / `tuple-enum` 报错桩保留迁移诊断。Map/Set `.contains?` 与 Map `.includes?` 也暂留：Map/Set 的 `.contains?` 同时来自 `Contains` trait impl，`src/calcit/type_annotation.rs` 的内建 trait 元数据（`builtin_core_trait_names`）列出 Map/Set 的 `Contains`，删除 impl 会让 `bootstrap_core_trait_names_match_core_impl_definitions` 失败；该文件另有并行改动，本次不碰，三者共用 `core-predicate-method-v1`，整组留到后续版本。

实现：

- core 方法表删除对应行；删除只服务 Map `.add` 的 `&map:add-entry`。
- `retired_method_migration` 为每个删除的方法给出 `E_RETIRED_METHOD` 与首选写法，只按 List/Map/Set 接收者类型匹配，用户自定义同名 trait 方法不受影响。
- fix：6 条规则的全部内容随旧方法消失（`core-set-include-v1`、`core-list-fold-v1`、`core-list-flat-map-v1`、`core-list-join-string-v1`、`core-list-get-v1`、`core-collection-combine-v1`），按 #1819 的做法整体退役：规则依赖旧方法仍能解析才可证明等价，目标 CLI 无法再匹配。`--rule` 选择这些 ID（以及更早退役的 `core-list-intersperse-v1`、`core-map-distinct-values-v1`）时报错并提示用 0.28.x 的 CLI 迁移。`core-api-0.28-v1` 由 13 条减为 7 条，`core-api-0.29-v1` 由 14 条减为 8 条；`core-predicate-method-v1` 保留。

兼容边界：List/String/Fn 的 `.mappend`、String/Enum 的 `.nth`、Fn 的 `.bind`、Map/Set/Struct/Enum 的 `.contains?`、Map `.includes?`、前缀 `reduce` / `join-str` / `contains?` 均不变。

验证：core `:tests` 改用首选写法；`strict_type_fail_retired_alias_methods_report_preferred_spelling` 覆盖 8 个新退役方法；`check-agent-interface` 断言 `query type` 不再列出这些方法；`cargo test`、`yarn check-all`、`yarn check-agent-interface`。
