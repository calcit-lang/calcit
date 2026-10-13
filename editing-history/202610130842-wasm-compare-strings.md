# WASM `&compare` 对字符串使用字典序

#2017 的类型化字符串排序在 WASM 上得到错误顺序。拆解后根因不在排序回调、Option 载荷或集合相等：
`first`、`option:unwrap-or`、`count` 以及 `&compare` 的数字输入都与 native 一致。
WASM 的 `&compare` 把两个操作数当作 f64 比较，字符串是堆指针，于是比较的是分配地址，
字面量恰好按出现顺序分配才碰巧正确，运行期构造的字符串则顺序随机，`assert=` 因而失败。

`&compare` 现在在运行期先确认两个操作数都在堆范围内且类型头为 string，是则调用已有的
`__rt_str_compare`（与 `&str:compare` 同一实现），否则保持原有数字三路比较与 NaN 规则。
头部只在指针范围检查通过后读取，普通数字不访问内存，与 `=` 的堆指针判定一致。
没有新增诊断、错误码、脚本或类型放宽；回调的参数与返回类型保持不变。

其他堆类型（tag、list 等）的 `&compare` 仍按数值处理，与 native 的跨类型全序不同，
这是已有的 WASM 限制，未在本次范围内扩大。

回归写成 definition `:tests`：`calcit.core/&compare#orders-runtime-built-strings` 与
`app.string-option/sort-date-strings` 的新增用例，覆盖空串、不同首字符、相同首字符与逐项
断言，并移除了 #2016 测试在 WASM 上的 parity 排除项。
