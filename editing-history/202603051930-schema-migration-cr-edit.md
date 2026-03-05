# 2026-03-05 19:30 schema 迁移记录

## 本次改动

- 持续使用 `cr edit schema` 迁移 `src/cirru/calcit-core.cirru` 中 `defn` 的 `:schema`。
- 统一命令形式为：
  - entry 使用 `src/cirru/calcit-core.cirru`
  - target 使用 `namespace/definition`
  - schema 使用 pair map 语法：`{} (:kind :fn) (:args ...) (:return ...)`
- 对包含 `?` 的函数名（如 `set?`）在 shell 中使用引号，避免 zsh 通配展开。

## 迁移经验

- `cr -- calcit/test.cirru edit schema calcit.core/...` 会触发“只允许 app 包编辑”，改用 core 源文件作为 entry 可编辑 core namespace。
- schema map 在 `-e` 中不能写成 `:args ... :return ...` 平铺键值，必须写成 `(:args ...)`、`(:return ...)` 的 pair。
- 批量链式命令中若某个目标失败，会中断后续目标；迁移后需要用搜索确认实际落盘范围。

## 本轮补充的 schema（代表性）

- `section-by`, `select-keys`, `set?`, `slice`, `some-in?`, `some?`
- `str`, `str-spaced`, `string?`, `strip-prefix`, `strip-suffix`
- `struct?`, `symbol?`, `syntax?`, `tag?`, `tagging-edn`
- `take`, `take-last`, `thread-step?`, `tuple?`, `turn-str`
- `union`, `unselect-keys`, `update`, `update-in`
- `option:map`, `optionally`, `pairs-map`, `range-bothway`, `result:map`, `vals`, `zipmap`
- `calcit.internal/normalize-trait-type`

## 验证

- 运行通过：`cargo test && yarn check-all`。
