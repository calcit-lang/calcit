## 概要

本次提交继续推进 schema-first 迁移，聚焦 `src/cirru/calcit-core.cirru`，为核心宏入口补齐显式 `:schema`，在不改变运行逻辑的前提下提升静态信息覆盖率与一致性。

## 关键改动

- 补齐高频线程/控制宏 schema：`->`、`->>`、`<-`、`if-not`、`thread-as`、`thread-first`、`thread-last`、`;nil`、`:`。
- 补齐构造/模式宏 schema：`list-match`、`record-match`、`record-with`、`tag-match`、`field-match`、`{}`。
- 补齐内部宏入口 schema：`&case`、`&list-match-internal`、`&record-match-internal`、`&field-match-internal`。
- 补齐工具/调试宏 schema：`[,]`、`[][]`、`\`、`\.`、`call-w-log`、`call-wo-log`、`js-object`、`w-log`、`w-js-log`、`with-cpu-time`、`with-gensyms`、`wo-log`、`wo-js-log`、`noted`。

## 结果

- `calcit-core` 中 `defmacro` 对应的 `:schema nil` 已清空。
- 每批改动后均执行 `edit format` 与 `yarn check-all`，回归通过。

## 经验

- 采用“按语义分组小批次补齐 + 每批全量回归”策略，可在大文件中稳定推进迁移并快速定位异常。
- 对宏统一使用 `:kind :macro` 与 `:args/:rest/:return` 结构，能减少后续工具链处理分支。