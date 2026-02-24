# Guidebook No-Check Block Upgrades (2026-02-25)

## 概要

对 Calcit guidebook (`/Users/chenyong/repo/calcit-lang/guidebook`) 中的代码块进行系统性升级，
将 `cirru.no-check` 标记替换为可验证的 `cirru` 或 `cirru.no-run` 块。

## 修改范围

### 1. `docs/features/static-analysis.md`

**双重求值 Bug 修复**：
- 原始代码：`assert= a ([] "b" 2 "a" 2)` 使用了导致误报的冗余赋值写法。
- 修复为直接 `assert= (get xs :xs) ys` 模式，规避了 `$ (expr)` 双重求值陷阱。
- 将 4 个 `cirru.no-check` 块正确转换为可运行/可类型检查的 `cirru` 块。

**核心知识点**：在 `let` 绑定中 `x $ (f a)` 会将 `(f a)` 的结果再作为操作符调用，
必须改为 `x (f a)` 或省略 `$` 的形式。

### 2. `docs/features/tuples.md`

**伪代码替换**：
- 将 5 个含 `&tuple:with-class` 调用的 `cirru.no-check` 块全部移除或替换。
- `&tuple:with-class` 已从语言中删除，不再作为公共 API 存在。
- 涉及内部机制的 3 个伪代码块改为 `text` 或 `code` 块并加注释。
- 保留可运行的 `cirru` 示例块（使用 `defrecord` / `defenum` 实际语法）。

### 3. `docs/features/common-patterns.md`

- 将 3 个 `cirru.no-check` 块改为 `cirru.no-run`（语法正确但不运行的说明性代码）。
- 涉及 `println` 副作用型示例以及占位符 `...` 形式的模板代码。

### 4. 其他文档

- `docs/features/sets.md`, `docs/data/persistent-data.md`, `docs/features/hashmap.md` 等多处：
  - 将说明性 `cirru.no-check` 改为 `cirru.no-run`（语法正确、无副作用但不执行）。
  - 将可验证的示例改为 `cirru`（加入 `assert=` 或 `assert` 验证）。

## 测试结果

全量测试通过：`yarn check-all` → 188/188 ✅

## 相关知识点

- **`cirru.no-check` vs `cirru.no-run` vs `cirru`**：
  - `cirru`：完全运行并类型检查，推断语法语义。
  - `cirru.no-run`：语法和类型检查，但不执行（适合副作用或模板代码）。
  - `cirru.no-check`：完全跳过检查（仅在代码不可验证时使用，如调用不存在的 API）。
- **`check-md` 子命令**：`cr docs check-md <file>` 用于单文件验证，加快 debug 循环。
- **双重求值陷阱**：`let` 中 `x $ (f a)` 等价于 `x ((f a))`，结果被再次调用，触发 "cannot be used as operator" 错误。
