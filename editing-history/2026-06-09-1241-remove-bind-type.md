# 2026-0609-1241 移除 bind-type（Breaking Change）

## 概要

彻底移除 `bind-type` 内置过程，所有类型槽绑定统一使用 `with-type-slot`。

## 动机

`bind-type` 是全局绑定，每个槽只能绑定一次，多入口项目（client/server）会产生冲突或需要分程序加载。`with-type-slot` 提供了基于词法作用域的替代方案，在入口函数体内局部绑定，天然支持多入口并行编译。

## 删除范围

### Rust 核心

| 文件 | 改动 |
|------|------|
| `src/calcit/proc_name.rs` | 删除 `BindType` 枚举变体及其 `ProcTypeSignature` |
| `src/calcit/type_annotation.rs` | 删除 `bind_type_slot()` 函数（`TYPE_SLOTS` 仍保留用于 `deftype-slot` 声明） |
| `src/calcit.rs` | 删除 `bind_type_slot` 的 re-export |
| `src/builtins.rs` | 删除 `BindType => meta::bind_type(args)` dispatch arm |
| `src/builtins/meta.rs` | 删除 `bind_type()` 函数及其 `bind_type_slot` 导入 |
| `src/runner/preprocess/mod.rs` | 删除 `BindType` 预处理块（~90 行）及 `precompile_bind_type_defs()` 函数 |
| `src/lib.rs` | 删除 `precompile_bind_type_defs` 调用 |
| `src/program.rs` | 删除 `calcit_contains_bind_type()` 及 snapshot 任务排序中的优先级逻辑 |
| `src/codegen/emit_js.rs` | 从 JS codegen no-op 分支中移除 `BindType` |

### 测试与固件

| 文件 | 改动 |
|------|------|
| `calcit/type-fail/type-slot-bind-duplicate.cirru` | **删除**（测试已无法成立） |
| `calcit/type-fail/type-slot-bind-unknown.cirru` | **删除**（`with-type-slot` 不需要预先声明） |
| `calcit/type-fail/type-slot-enum-invalid-variant.cirru` | `bind-type` → `with-type-slot (:dispatch-op Action)` |
| `calcit/type-fail/type-slot-record-call-arg-type-mismatch.cirru` | `bind-type` → `with-type-slot (:payload ...)` |
| `calcit/type-fail/type-slot-entry-scope.cirru` | `bind-type` → `with-type-slot` |
| `src/bin/cr_tests/type_fail.rs` | 删除 `type_fail_type_slot_fixtures_report_errors` 测试；更新注释 |

### 文档

| 文件 | 改动 |
|------|------|
| `docs/features/static-analysis.md` | "Binding a Type Slot" 章节改用 `with-type-slot` 说明 |

### 外部项目

| 文件 | 改动 |
|------|------|
| `calcium-workflow/calcit.cirru` (`app.server/main!`) | `(bind-type :dispatch-op Op)` → `with-type-slot (:dispatch-op Op)` |

## 验证

- `cargo build --bin cr` — 零警告、零错误
- `cargo clippy -- -D warnings` — 通过
- `cargo test` — 85 个测试全部通过
- `calcium-workflow --check-only`（client + server 入口）— 均通过

## 迁移指南

将所有 `bind-type :slot-name TypeExpr` 替换为：

```cirru
with-type-slot (:slot-name TypeExpr)
  ;; 原来在 bind-type 之后的代码全部作为 body
```
