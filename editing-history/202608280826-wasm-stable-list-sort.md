# WASM stable list sort / WASM 稳定列表排序

## 中文

- 用稳定 insertion sort 替换 `sort` 与 `&list:sort` 的静默 identity stub。
- 排序前复制线性内存 list，保持输入不可变；comparator 复用 static、inline、native proc 与动态函数表调用路径。
- comparator 结果仅在大于零时移动左值，因此 equal/NaN 保持原顺序。
- 一参数 heterogeneous total-order 明确报 unsupported，不再生成语义错误的 WASM。
- 增加升序、降序、重复 key 稳定性、输入不可变、动态 comparator 与 unsupported 路径测试。

## English

- Replace the silent identity stubs for `sort` and `&list:sort` with stable insertion sort.
- Copy the linear-memory list before sorting to preserve input immutability, reusing static, inline, native-proc, and dynamic function-table comparator paths.
- Move the left value only when the comparator is greater than zero, preserving order for equal and NaN results.
- Reject the one-argument heterogeneous total-order form explicitly instead of generating semantically incorrect WASM.
- Add ascending, descending, duplicate stability, input immutability, dynamic comparator, and unsupported-path tests.
