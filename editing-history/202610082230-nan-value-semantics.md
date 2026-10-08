# NaN 的相等、排序与哈希语义（#1650）

问题：native `Eq` 用 `a == b`（`NaN != NaN`），`Ord` 只比较 `<` / `>`（`NaN` 与任何数字都 `Equal`），违反 Rust `Eq` / `Ord` 约定；`sort` 对含 `NaN` 的列表不能给出有序结果，`NaN` 放进 Set/Map 后无法找回。JS 的数字哈希 `31 * base + NaN` 得到 `NaN`，哈希桶永远不命中；WASM 的 `=`、Set/Map 查找都是 `f64.eq`，哈希不规范化 `NaN` 的位模式。

决策：采用 issue 中的方案 A。

- 值相等：`NaN` 等于 `NaN`，`0` 等于 `-0`。
- 全序：`-inf < ... < -0 == 0 < ... < inf < NaN`。不直接用 `f64::total_cmp`，因为它区分 `-0` 与 `0`，与值相等不一致。
- 哈希：所有 `NaN` 共享一个哈希；`0` 与 `-0` 共享一个哈希（native 已如此，JS 与 WASM 补齐）。
- `<`、`>`、`<=`、`>=` 保持 IEEE 754：它们是数值运算，不是值排序；需要排序用 `&compare`。
- 方案 B（产生 NaN 即报错）与 JS 互操作成本高，不采用。

实现：native 在 `src/calcit.rs` 提供 `number_value_eq` / `number_total_cmp` 供 `PartialEq` / `Ord` 使用；JS 修改 `_$n__$e_`、`_$n_compare` 与数字哈希；WASM 新增运行时 `__rt_f64_value_eq`，替换 `=` 快路径、`__rt_value_equal`、Set/Map 键查找与 Map 值查找中的 `f64.eq`，`__rt_hash_f64` 规范化 `NaN`，`&compare` 在 `NaN` 参与时返回 `isnan(a) - isnan(b)`。

兼容边界：`(= x x)` 不再能区分 `NaN`，core 目前没有专门的 NaN 谓词，可用 `&= 1 $ &compare x $ &/ 1 0`；排序结果中 `NaN` 现在固定排在最后。

验证：core `:tests` `&compare#orders-nan-last`、`=#treats-nan-as-equal-value`、`sort#sorts-nan-after-numbers`（WASM 不支持默认比较器，已排除）、`sort#sorts-nan-last-with-compare`、`contains?#finds-nan-keys` 在 native / JS / WASM 通过；`docs/data/number.md` 记录规则。
