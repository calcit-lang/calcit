# 退场 .join、Map .values 与 List/String .contains?

按 `docs/run/upgrade.md` 的退场节奏，在 0.28.0 之后的第一个非 patch 版本（0.29.0）删除三组旧方法。维护者在线程中明确允许承担下游风险；实际的下游清零证据没有取得（fix 预览在 Respo main 上因 #1694 的消费者问题无法预处理，语法搜索不能区分 core 自身命中），因此升级说明把迁移绑定在 0.28.x 的 CLI 上。

范围：core 的 List `:join`、Map `:values`、List/String `:contains?` 方法条目，List/String 的 `Contains` trait 实现，以及预处理对 `("contains?", List|String)` 的内联 lowering。函数形式 `contains?` 保留，改为对 List/String 直接调用 `&list:contains?` / `&str:contains?`，对 Map/Set/Struct/Enum 仍走 `.contains?`。Map/Set/Struct/Enum 的 `.contains?`、前缀 `join` / `vals` 不变。

诊断：旧调用在 Rust 预处理里没有静态错误，只会落到“cannot be used as operator”之类的通用失败，因为未知方法名不走类型化的方法路径。新增 `retired_method_migration`：接收者类型能证明是 List/Map/String 且方法名在表内时，仍进入类型化方法路径，由 `validate_method_call` 给出 `E_RETIRED_METHOD` 与首选替代。动态接收者保持原行为，不可能静态报告。

fix 规则：旧方法不存在后，`core-list-intersperse-v1`、`core-map-distinct-values-v1` 和 `core-predicate-method-v1` 的 List/String 部分没有可证明的旧新方法对，保留只会误导，随旧方法一起退役；`core-api-0.28-v1` preset 因此由 15 条减为 13 条（其余规则不变，旧 surface preset 不受影响）。对应的 7 个 fix_cli 测试删除或收窄，Map/Set 的谓词规则测试保留。

验证：core `:tests` 在 native 449 项通过，JS（try-js）与 WASM（test-wasm.sh）通过；四种旧写法在严格检查下各报 `E_RETIRED_METHOD`；check-agent-interface、check-predicate-method-names、core-api-contract、Dynamic 分类基线与文档检查通过。
