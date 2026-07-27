# `&record:nth` — Compile-Time Record Field Index Optimization

## 概述

新增 `NativeRecordNth` / `&record:nth` 过程，在预处理阶段将 record 字段访问优化为 O(1) 索引访问，绕过运行时的类型分发链和二分查找。

## 优化前访问路径

- `(:field record)` → 重写为 `(get arg :field)` → 运行时 6 次类型判断 (`nil?` → `string?` → `map?` → `list?` → `tuple?` → `record?`) → `&record:get` → `index_of()` O(log n) 二分查找
- `&record:get record :field` → 运行时 `index_of()` O(log n) 二分查找

## 优化后访问路径

当预处理阶段可以通过 `resolve_type_value` + `as_struct()` 解析出 record 的 struct 定义时：

- `(:field record)` → 直接重写为 `(&record:nth record <compile-time-idx>)` → 运行时 O(1) `values[idx]`
- `&record:get record :field` → 重写为 `(&record:nth record <compile-time-idx>)` → 运行时 O(1)

## 触发条件

优化仅在类型信息明确时激活：
- record 由 `%Struct ...` 构造器在本地 `let` 中创建
- 函数参数有显式 struct 类型的 schema 标注

若类型不明，回 fallback 到原有路径，不影响运行时行为。

## 修改文件

1. **`src/calcit/proc_name.rs`** — 新增 `NativeRecordNth` 变体 (`&record:nth`)，签名 `(record, number) → dynamic`
2. **`src/builtins/records.rs`** — 实现 `record_nth(xs)` 运行时函数，直接 `values[idx]` 访问
3. **`src/builtins.rs`** — 分发表连线 `NativeRecordNth => records::record_nth`
4. **`src/runner/preprocess/mod.rs`** — 两个重写点：
   - Tag-call (`Calcit::Tag` as head)：解析 arg 类型后索引重写
   - `NativeRecordGet` proc call：解析 record arg 类型后索引重写
5. **`src/runner/preprocess/type_inference.rs`** — `infer_record_nth_type()` 根据 struct 的 `field_types[idx]` 推断返回类型
6. **`src/runner/preprocess/type_checking.rs`** — 将 `NativeRecordNth` 加入 core arg type check 跳过列表
7. **`src/codegen/emit_js.rs`** — JS 内联代码生成 `record.values[idx]`，无函数调用开销

## 知识点

- `CalcitStruct.index_of(field_str)` 在编译期使用，用字段名解析排序位置
- `resolve_type_value()` 返回 `None` 时优化不触发，安全回退
- JS CalcitRecord 的 `.values` 数组和 Rust 端的 `values` Vec 索引一致（均按字段名字母序排序）
- 对于真实项目（如 respo），函数参数通常缺少 struct 类型标注，优化暂不触发；后续若 schema 推断增强，无需改动 rewrite 逻辑即可自动生效
