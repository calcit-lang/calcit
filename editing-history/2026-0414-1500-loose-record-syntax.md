# 2026-04-14 15:00 — `?{}` 松散 Record 语法

## 概要

新增 `?{}` 松散 record 语法，允许不声明 struct 就创建 record，与 `::` 无类型 tuple 对称。
在函数参数有 struct 类型标注时，预处理阶段自动改写为 `%{} StructDef ...`。

## 改动文件

| 文件 | 变更 |
|------|------|
| `src/calcit/proc_name.rs` | 新增 `NativeLooseRecord` proc 变体，`?{}` 序列化，类型签名 |
| `src/data/cirru.rs` | `"?{}"` token 解析为 `Calcit::Proc(NativeLooseRecord)` |
| `src/calcit/record.rs` | `LOOSE_RECORD_NAME`、`is_loose()`、`from_loose_pairs()` |
| `src/builtins/records.rs` | `call_loose_record()` 运行时：验证 tag 键、排序、排重 |
| `src/builtins.rs` | 分发 `NativeLooseRecord => call_loose_record(args)` |
| `src/calcit.rs` | Display: 松散 record 显示为 `(?{} ...)` |
| `src/runner/preprocess.rs` | `try_rewrite_loose_record_args_to_struct_records`、类型推断、skip core arg check |
| `ts-src/js-record.mts` | JS 运行时 `_$q__$M_` 函数 |
| `calcit/test-record.cirru` | `test-loose-record-rewrite` 集成测试 |
| `docs/features/records.md` | 文档：松散 record 语法、自动改写、类型对称表 |

## 性能优化

- **预处理改写链合并重建**：三步改写（map→record、loose→struct、tuple→enum）原先每步都重建 `ys` (CalcitList)，优化为仅在有改写时统一重建一次，减少 CalcitList push 操作。
- **JS 排序比较器修正**：将重复字段检测从 sort comparator 中移出（comparator 内 throw 违反排序契约），改为排序后线性扫描相邻元素，行为更可靠。

## 知识点

- `EdnTag::cmp` 按全局 tag 池分配顺序排序（整数 index），**不是**字典序。record 字段排序必须用 `ref_str().cmp()` 保持字母序。
- CalcitList 基于 persistent ternary tree，`.clone()` 是 O(1) 引用计数操作。
- Sort comparator 不应抛异常——不同排序算法对异常行为未定义。
