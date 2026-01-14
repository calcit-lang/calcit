# Enum Tuple Runtime Validation - Session Summary

## 已完成的工作

### 1. Rust 侧实现

- ✅ **新增函数** (`src/builtins/meta.rs`):
  - `new_enum_tuple()`: `%%::` 操作符，创建带 enum 元数据的 tuple 并验证
  - `tuple_enum()`: 获取 tuple 的 enum 原型
  - `tuple_enum_has_variant()`: 检查 enum 是否有指定 variant
  - `tuple_enum_variant_arity()`: 获取 variant 的参数个数
- ✅ **验证逻辑**: 在创建时检查 tag 是否存在于 enum，arity 是否匹配
- ✅ **单元测试**: 23 个 Rust 单元测试全部通过

### 2. TypeScript 侧实现

- ✅ **CalcitTuple 类增强** (`ts-src/js-tuple.mts`):
  - 添加 `enum` 属性存储 enum 原型
  - 更新构造函数和 `assoc` 方法
- ✅ **新增导出函数** (`ts-src/calcit.procs.mts`):
  - `_$n_tuple_$o_enum()`: 获取 enum 原型
  - `_$n_tuple_$o_enum_has_variant()`: 检查 variant 存在性
  - `_$n_tuple_$o_enum_variant_arity()`: 获取 variant arity
  - `_PCT__PCT__$o__$o_()`: `%%::` 操作符实现
- ✅ **代码优化**: 使用 CalcitRecord 的 `contains()` 和 `getOrNil()` 方法

### 3. 核心库更新

- ✅ **函数定义** (`src/cirru/calcit-core.cirru`):
  - 添加 `&tuple:enum`, `&tuple:enum-has-variant?`, `&tuple:enum-variant-arity` 标记为 `&runtime-inplementation`

### 4. 测试状态

- ✅ 完整测试套件（`yarn check-all`）全部通过
- ✅ Rust 解释器和 JavaScript 编译模式都正常工作

---

## 发现的核心问题 ⚠️

### 问题：EDN 反序列化丢失 enum 元数据

**现象：**
从 `compact.cirru` 加载的代码中，tuple 的 enum 元数据丢失。

**根本原因：**
在 `src/data/edn.rs:132`，EDN 反序列化时硬编码设置：

```rust
Edn::Tuple(EdnTupleView { tag, extra }) => Calcit::Tuple(CalcitTuple {
  tag: Arc::new(edn_to_calcit(tag, options)),
  extra: extra.iter().map(|x| edn_to_calcit(x, options)).collect(),
  class: None,
  sum_type: None,  // ← 问题所在！
}),
```

**影响范围：**

- ✅ **直接调用有效**: REPL、`cr eval` 中使用 `%%::` 会正常验证
- ❌ **文件加载失效**: 从 snapshot 加载的代码，tuple 的 `sum_type` 为 `None`
- ❌ **tag-match 验证无效**: 因为无法获取 enum 元数据，所以 tag-match 中的验证被跳过

**验证流程对比：**

| 场景                         | enum 元数据 | 验证效果          |
| ---------------------------- | ----------- | ----------------- |
| `cr eval "(%%:: C E :ok 1)"` | ✅ 保留     | ✅ 创建时验证有效 |
| 从 compact.cirru 加载        | ❌ 丢失     | ❌ 无法验证       |

**证据：**
运行测试时 tuple 显示 `(%%:: :ok 42 (:class ResultClass) (:enum Result))`，说明序列化时保存了 enum 信息（见 `src/codegen/gen_ir.rs:380`），但反序列化时没有恢复。

---

## 未来改进计划

### 短期方案（已实现）

- ✅ 依赖创建时验证（`%%::` 调用时）
- ✅ 移除了 tag-match 中无效的验证代码
- ✅ 确保现有功能正常工作

### 中期方案（待实现）

**修复 EDN 反序列化，恢复 enum 元数据**

需要修改的文件：

1. `src/data/edn.rs` - 反序列化逻辑

   - 解析 tuple 的 `:enum` 标签
   - 在 program/snapshot 中查找对应的 enum 定义
   - 恢复 `sum_type` 字段

2. 可能的实现思路：

   ```rust
   // 伪代码
   Edn::Tuple(EdnTupleView { tag, extra }) => {
     let mut tuple = CalcitTuple {
       tag: Arc::new(edn_to_calcit(tag, options)),
       extra: extra.iter().map(|x| edn_to_calcit(x, options)).collect(),
       class: None,
       sum_type: None,
     };

     // 检查是否有 :enum 标签
     if let Some(enum_name) = get_enum_tag(&extra) {
       // 从 program 中查找 enum 定义
       if let Some(enum_def) = program.find_enum(enum_name) {
         tuple.sum_type = Some(Arc::new(enum_def));
       }
     }

     Calcit::Tuple(tuple)
   }
   ```

3. 挑战：
   - 需要访问 program/snapshot 上下文
   - EDN 解析器目前是无状态的
   - 可能需要重构 `edn_to_calcit` 函数签名

### 长期方案（可选）

**静态类型检查**

- 在编译/preprocessing 阶段检查 enum 使用
- 无需依赖运行时元数据
- 需要类型推导系统支持

---

## 当前状态

### 可用功能

- ✅ `%%:: Class Enum :tag payload...` 创建 enum tuple 并验证
- ✅ `&tuple:enum tuple` 获取 enum 原型（仅运行时创建的有效）
- ✅ `&tuple:enum-has-variant? enum tag` 检查 variant
- ✅ `&tuple:enum-variant-arity enum tag` 获取 arity

### 已知限制

- ⚠️ 从文件加载的代码无法进行 enum 验证
- ⚠️ tag-match 中无法检查 enum 信息
- ⚠️ 需要用户确保使用 `%%::` 创建 enum tuple

### 测试文件

- `calcit/test-sum-types.cirru` - 基本 enum 功能测试
- `calcit/test-enum-validation.cirru` - 创建时验证测试
- `calcit/test-tag-match-validation.cirru` - tag-match 验证测试（当前受限）

---

## 相关文件

### 核心实现

- `src/builtins/meta.rs` - Rust 侧函数实现
- `src/calcit/proc_name.rs` - Proc 枚举定义
- `src/builtins.rs` - 内置函数映射
- `src/cirru/calcit-core.cirru` - Calcit 核心库定义
- `ts-src/js-tuple.mts` - TypeScript Tuple 类
- `ts-src/calcit.procs.mts` - TypeScript 导出函数

### 需要修改的文件

- `src/data/edn.rs` - EDN 反序列化（待修复）
- `src/codegen/gen_ir.rs` - IR 生成（已正确序列化 enum）

---

## 后续步骤

1. **确认需求优先级**：

   - 是否需要立即修复 EDN 反序列化？
   - 或者当前的创建时验证已经足够？

2. **如果修复 EDN**：

   - 研究 `edn_to_calcit` 函数调用链
   - 设计上下文传递机制
   - 实现 enum 定义查找
   - 添加测试验证

3. **文档更新**：
   - 说明 enum tuple 的使用方式
   - 注明当前限制
   - 提供最佳实践指南

---

**最后更新**: 2026-01-15
**当前分支**: enum-validation
**测试状态**: ✅ 全部通过 (yarn check-all)
