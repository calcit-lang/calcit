# 2026-0223-0834 — 迁移到 defstruct 方案并实现 `%{}?` 可选字段 Record 宏

## 背景

将 Calcit Record 系统从旧的 `new-record`/`defrecord`/`defrecord!` 方案全面迁移到基于 `defstruct` 的新方案，同时新增 `%{}?` / `&%{}?` 支持可选字段初始化与更新。

---

## 知识点一：`%{}?` 与 `&%{}?` 的语义设计

`struct` 初始化为 record 时，部分字段在语义上是可选的，对应 TypeScript 中的 `{ a?: number; b?: number }`。原有的 `%{}` / `&%{}` 要求所有字段都必须显式传入。

### `%{}?`（macro）

- 初始化 record 时允许省略字段，省略的字段自动填 `nil`。
- 用法：`%{}? MyRecord (:x 1)`

### `&%{}?`（proc）

`call_record_partial` 的语义：

- **proto 是 Struct**：以全 `nil` 为基础 `values`，用传入的 k-v 覆盖对应位置。
- 传入未知字段 → 报错；传入重复字段 → 报错；`(args_size - 1) % 2 != 0` → 报错。

关键实现细节：`CalcitStruct.fields` 元素类型是 `EdnTag`，比较时用 `f.ref_str()` 而非 `f.as_ref()`（后者无 `AsRef` impl）。

### `%{}?` 宏定义

```cirru
defmacro %{}? (R & xs)
  if
    not $ and (list? xs) (every? xs list?)
    raise $ str-spaced "|%{}? expects field entries in list, got:" xs
  &let
    args $ &list:concat & xs
    quasiquote $ &%{}? ~R ~@args
```

---

## 知识点二：Rust proc 注册流程

新增一个内置 proc 需要同时修改四处：

| 文件                       | 修改内容                                                 |
| -------------------------- | -------------------------------------------------------- |
| `src/calcit/proc_name.rs`  | 添加枚举变体 `NativeRecordPartial` + `ProcTypeSignature` |
| `src/builtins/records.rs`  | 实现 `call_record_partial` 函数                          |
| `src/builtins.rs`          | 在 `match proc` 中添加分发分支                           |
| `src/runner/preprocess.rs` | 将新 proc 加入"跳过 arity 检查"的 `matches!` 列表        |

---

## 知识点三：`%{}` 严格要求 Struct proto

- `call_record`（`%{}`）：若 proto 为 `Calcit::Record`，直接返回错误提示改用 `defstruct`。
- `defstruct` 产生 `Calcit::Struct`（含 `field_types`），并作为 `%{}` / `%{}?` 的唯一原型来源。

### `&record:get-name` / `&record:struct` 的参数约束

- 两者统一要求传入 `record`，避免 `struct` / `record` 混用语义。

### `&record:matches?` 的参数约束

- 第一参数要求 `record`。
- 第二参数接受 `record` 或 `struct`（用于 `record-match` 的模式匹配场景）。

---

## 修改文件

| 文件                          | 变更内容                                                                                                                                           |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/calcit/proc_name.rs`     | 新增 `NativeRecordPartial` 枚举变体与类型签名；移除 `NewRecord` 变体                                                                               |
| `src/builtins/records.rs`     | 新增 `call_record_partial`（struct-only）；`matches` 调整为 `(record, record/struct)`；移除 `new_record` 函数 |
| `src/builtins.rs`             | 分发 `NativeRecordPartial`；移除 `NewRecord` 分发                                                                                                  |
| `src/runner/preprocess.rs`    | arity 检查豁免新增 `NativeRecordPartial`                                                                                                           |
| `src/cirru/calcit-core.cirru` | 新增 `%{}?` 宏定义与 `&%{}?` 文档条目；`defrecord`/`defrecord!` 改为 raise error；删除 `new-record` 定义                                           |
| `calcit/test-record.cirru`    | Cat/BirdShape/Person/City/A/B/C/Demo 全部改为 `defstruct`；删除所有 `new-record` let 绑定；修复各测试函数体                                        |
| `docs/CalcitAgent.md`         | 类型标注示例中 `new-record` 改为 `defstruct`                                                                                                       |

---

## 验证

```bash
cargo run --bin cr -- calcit/test-record.cirru -1
cargo clippy -- -D warnings
```

全部通过，无 warning。
