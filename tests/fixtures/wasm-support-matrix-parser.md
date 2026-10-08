# 诊断表解析测试

正文中的 `E_WASM_FIRST` 不能代替诊断表条目。

## 表层构造

| 编号 | 含义 | 处理方式 |
| --- | --- | --- |
| `E_WASM_OTHER_TABLE` | 不在诊断章节中 | 不计入 |

## 诊断

即使正文提到 `E_WASM_PROSE_ONLY`，也不算已经登记。

| 编号 | 含义 | 处理方式 |
| --- | --- | --- |
| `E_WASM_FIRST` | 说明引用 `E_WASM_DESCRIPTION_ONLY` | 建议引用 `E_WASM_FIX_ONLY` |
| `E_WASM_SECOND` / `E_WASM_THIRD` | 同一格可以有多个编号 | 已登记 |

表后正文 `E_WASM_AFTER_TABLE` 也不计入。

## 维护

| 编号 | 含义 | 处理方式 |
| --- | --- | --- |
| `E_WASM_MAINTENANCE` | 不在诊断章节中 | 不计入 |
