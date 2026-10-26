# 断言证明下 assert= 比较 Option 不再误报旧可空用法

#2121 原型把普通 `--check-only` 切到断言证明后，生态 25 个仓库里只有 calcit.std 多出失败：
`assert= (Option :none) (rand-nth ...)` 报 `W_NOMINAL_ENUM_LEGACY_USE`。`assert=` 展开为
`&= (unsafe-coerce va 'Dynamic) (unsafe-coerce vb 'Dynamic)`；断言证明按设计不信任 coercion 合同，
于是右侧又被看成 `get`/`rand-nth` 返回的 Option，左侧字面构造看不出 Option，检查误判为
「Option 与 payload 比较」。

修正：旧可空用法检查把显式 `unsafe-coerce x 'Dynamic` 当作有意擦除，不计入 nominal 实参。
这只影响该 lint；证明本身仍不信任 coercion 合同。普通检查下 coercion 结果本来就是 Dynamic，
行为不变。

回归：`assertion_proof_keeps_assert_equal_option_erasure_quiet`，修前失败。

English: the nominal-enum legacy-use lint now treats an explicit `unsafe-coerce x 'Dynamic`
as a deliberate erasure, so `assert=` on Option values no longer warns under assertion proof.
