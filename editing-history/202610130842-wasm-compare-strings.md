# WASM `&compare` 对字符串使用字典序

#2017 的类型化字符串排序在 WASM 上得到错误顺序。拆解后根因不在排序回调、Option 载荷或集合相等：
`first`、`option:unwrap-or`、`count` 以及 `&compare` 的数字输入都与 native 一致。
WASM 的 `&compare` 把两个操作数当作 f64 比较，字符串是堆指针，于是比较的是分配地址，
字面量恰好按出现顺序分配才碰巧正确，运行期构造的字符串则顺序随机，`assert=` 因而失败。

`&compare` 现在逐个操作数判定是否为字符串：值必须是落在已分配堆内的整数，且 ptr-8 处为
`HEAP_MAGIC`、ptr-4 处为 string 类型标签，判定方式与 `type-of` 一致。仅在整数与范围检查
通过后才读内存，因此落在堆地址范围内的非整数（如 16392.5）仍按数字比较。两者都是字符串时
调用已有的 `__rt_str_compare`（与 `&str:compare` 同一实现）；只有一方是字符串时字符串更大，
与 native 的数字小于字符串一致；否则保持原有数字三路比较与 NaN 规则。
没有新增诊断、错误码、脚本或类型放宽；回调的参数与返回类型保持不变。

已知限制：tag、list 等其他堆类型之间的 `&compare` 仍按数值处理，与 native 的跨类型全序不同，
已写入 docs/installation/wasm-support.md 的限制。后续事项：`=` 的堆指针判定同样只做范围检查，
没有 magic 与整数校验，存在相同的误判风险，应另开 issue 处理，本次不改动。

回归写成 definition `:tests`：`calcit.core/&compare` 的运行期字符串、堆地址范围内的小数
（`&compare 16392.5 16392.75`，16392 为 HEAP_BASE+8）与数字先于字符串用例，以及
`app.string-option/sort-date-strings` 的新增用例（空串、不同首字符、相同首字符与逐项断言），
并移除了 #2016 测试在 WASM 上的 parity 排除项。

无 NaN-boxing 时，恰好等于某个存活堆对象地址的整数仍无法与指针区分，`type-of` 与
`=` 有同样的限制；本次不引入新的值表示，回归用例因此使用小数而非整数地址。
