# 2026-03-06 15:52 core schema + hint-fn 迁移批次

## 背景

本批次延续 schema-first 迁移：

- 在 `src/cirru/calcit-core.cirru` 继续将 runtime 内建的 `:schema nil` 改为显式 schema。
- 在 `calcit/` 测试目录中，将旧式 `hint-fn` 参数写法迁移为新写法。

## 主要改动

### 1) Core runtime schema 补齐（无行为改动）

在 `src/cirru/calcit-core.cirru` 增补了多批低风险条目的 schema，覆盖例如：

- 数学/比较/位运算：`cos`/`sin`/`sqrt`/`round`/`round?`/`floor`/`ceil`、`bit-*` 系列等
- 字符串/解析/格式化：`split`/`split-lines`/`trim`/`parse-cirru*`/`format-cirru*`
- IO 与环境：`read-file`/`write-file`/`get-env`/`generate-id!`
- 运行时工具：`atom`/`add-watch`/`remove-watch`/`type-of`/`turn-*`/`fold*`/`range`/`recur`/`raise`/`quit!`

原则：仅补 schema，不改运行时逻辑。

### 2) 旧式 hint-fn 参数写法迁移

在 `calcit/test-types-inference.cirru` 中，将旧写法：

- `:args $ [] :number`

迁移为新写法：

- `:args $ [] (:: 'x :number)`

并复查 `calcit/**` 下同模式，不再命中。

## 验证

每批次均执行：

- `cargo run --bin cr -- demos/calcit.cirru edit format`
- `yarn check-all`

结果均通过（尾部稳定为 `... and 24 files not changed.`）。

## 备注

- 当前提交聚合了本轮连续小批次迁移结果，便于后续按文件/功能继续清理剩余 `:schema nil`。

## 增补（同批次续改）

### 3) 移除 legacy hint-fn clause 兼容（改为直接报错）

在 `src/runner/preprocess.rs` 中将旧语法兼容从 warning 升级为 hard error：

- 不再接受 `hint-fn` 内 legacy clauses：`return-type` / `generics` / `type-vars`
- 统一在 `preprocess_hint_fn` 阶段报 `Syntax` 错误，提示迁移到 schema map 形式
- 删除旧的 `warn_on_legacy_hint_fn_syntax` 路径并同步测试

### 4) 文档更新（docs + guidebook）

同步更新示例与说明，避免继续传播旧写法：

- `docs/CalcitAgent.md`：补充“legacy clause 会直接报错”，并将 `:args` 示例改为命名参数条目
- `guidebook/docs/cirru-syntax.md`、`guidebook/docs/features/static-analysis.md`、`guidebook/docs/features/records.md`：统一 `hint-fn` 示例为 schema-map + `(:: 'arg <type>)` 形式