# 202604161520 - WASM 位运算 & match 语法编译

## 位运算 (bitwise operations)

- 新增 `bit-and`, `bit-or`, `bit-xor`, `bit-not`, `bit-shl`, `bit-shr` 6 个 WASM 编译支持
- 策略: f64 → i32 (trunc_f64_s) → 执行 i32 位运算 → i32 → f64 (convert_i32_s)
- 使用有符号转换 (I32TruncF64S / F64ConvertI32S) 保持负数语义

## match 语法编译

- 实现 `match` 表达式的 WASM codegen, 支持 enum tuple 模式匹配
- 策略: 加载 tuple 的 tag_id (offset 0), 对各分支做嵌套 if/else 比较
- 支持 tag 模式 `(:variant a b)` — 从 tuple 内存按偏移读取 payload 绑定到局部变量
- 支持 wildcard `_` 作为 fallback 分支
- 注意 block_depth 正确更新以保持 recur 分支跳转的一致性

## 测试覆盖

- 31 项 WASM 检查全部通过 (原 22 + 6 bitwise + 3 match)
- 246 cargo tests 通过 (179 + 67)
- clippy 零警告
