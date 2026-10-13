# WASM `=` 的堆值判定与 `type-of` 对齐（#2037）

WASM 的 `=`（`emit_equals_core_impl`）原先只检查值落在 `[HEAP_BASE+8, memory.size)` 内就当作堆指针，
随后读取 ptr-4 的类型标签。位于堆地址范围内的非整数 number（如 16392.5）会被截断后读内存，
可能把两个不同小数误判为同一对象（例如首个分配对象是字符串时走字符串比较）。

现在抽出共享的 `emit_is_heap_value`：先做廉价的范围检查（`>= HEAP_BASE+8` 且 `< HEAP_PTR`），
再要求整数，最后才读取 ptr-8 的 `HEAP_MAGIC`，与 `type-of` 的判定一致。`emit_is_string`
（#2035 的 `&compare` 使用）改为在其之上只追加字符串标签检查；`=` 对两侧各用一次该判定。
普通数字在 `=` 的快速路径之后只多做两次浮点范围比较与一次 trunc 比较，不读内存。

兼容边界：上界由 `memory.size` 收紧为当前 `HEAP_PTR`（与 `type-of` 相同），已分配对象均位于其下。
其余相等语义（字符串内容、list/map/set/enum 结构相等、NaN 视为相等）不变，没有新增诊断或类型放宽。
`&compare` 对 tag/list 等其他堆类型仍按数值比较，限制保持写在 docs/installation/wasm-support.md。
无 NaN-boxing 时恰好等于存活堆对象地址的整数仍无法与指针区分，本次不改变值表示。

回归：`calcit.core/=` 的 `:tests` 增加 `fractions-in-heap-address-range-stay-numbers`，
在 native 与 WASM 上复现。
