# 202604142259 — Bidirectional Type Checking Architecture Refactoring

## 概要

将 `src/runner/preprocess.rs`（6199 行单体文件）拆分为模块化目录结构，
按双向类型检查（bidirectional type checking）的概念分离关注点。

## 改动详情

### 模块拆分

`preprocess.rs` → `preprocess/` 目录模块：

| 模块 | 行数 | 职责 |
|---|---|---|
| `mod.rs` | ~4744 | 预处理核心：符号解析、宏展开、作用域管理、特殊语法 |
| `type_inference.rs` | ~693 | 类型合成（synthesis）：自下而上推导表达式类型 |
| `type_checking.rs` | ~402 | 类型检查（checking）：自上而下验证参数类型 |
| `type_rewriting.rs` | ~398 | 类型定向重写：根据期望类型重写 AST 字面量 |

### 代码去重

1. **统一参数重写循环** (`rewrite_args_by_expected_type`)：
   - 原先 4 个 `try_rewrite_*_args_to_*` 函数各自包含相同的外层循环（~30 LOC × 4）
   - 提取为泛型 `rewrite_args_by_expected_type<F>` 函数，4 个入口各只保留闭包调用

2. **统一参数类型检查循环** (`check_arg_types_loop`)：
   - 原先 `check_local_fn_call_arg_types` 和 `check_user_fn_arg_types` 共享相同的
     zip → resolve_type_value → matches_with_bindings → gen_check_warning 模式
   - 提取为泛型 `check_arg_types_loop<F>` 函数，包含 variadic 处理和 spread 跳过逻辑

3. **统一引用节点构建** (`build_struct_ref_node` / `build_enum_ref_node`)：
   - 原先 map→record 和 loose-record→struct 各自复制 Import vs Struct/Enum 分支
   - 提取为共享的 `build_struct_ref_node` 和 `build_enum_ref_node` 函数

### Proc 推导提取

- `infer_type_from_expr` 中的 `Calcit::Proc` 分支（~100 行 match arms）
  提取为独立的 `infer_proc_call_return_type` 函数，提高可读性

## 双向类型检查映射

参照 Dunfield & Krishnaswami 的框架：

| 方向 | 模块 | 入口函数 |
|---|---|---|
| **Synthesis** (⇒) | `type_inference` | `infer_type_from_expr`, `resolve_type_value` |
| **Checking** (⇐) | `type_checking` | `check_*_arg_types`, `check_function_return_type` |
| **Rewriting** (结构适配) | `type_rewriting` | `try_rewrite_*` |
| **Context** (线程局部) | `mod.rs` | `EXPECTED_FN_TYPE`, `EXPECTED_STRUCT_TYPE` |

## 为后续 match 语法扩展铺路

- `type_checking.rs` 的 `check_arg_types_loop` 模式可直接复用于 match 分支检查
- `type_inference.rs` 中已有 `infer_if_return_type` 作为分支合并的模板
- 未来只需在相应模块中添加新函数，无需修改 `mod.rs` 的核心调度

## 验证

- `cargo check --lib` ✅ 零警告
- `cargo clippy --lib -- -D warnings` ✅ 零警告
- `cargo fmt` ✅ 格式一致
- 链接器问题（`ld: library 'System' not found`）是环境问题，非代码变更引起
