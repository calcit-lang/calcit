# 202602182334 panic hardening summary

## 本次修改要点

- 将运行时多个 `unreachable!`/panic 路径替换为可恢复或可比较的分支，降低崩溃概率。
- `EntryBook::load` 改为返回 `Result`，调用侧改为容错回退。
- `record.extend_field` 在重复字段时返回错误，而不是 panic。
- `Calcit` 的集合/映射/记录比较补齐稳定比较逻辑。
- `AnyRef` 在 `cmp` 与 `hash` 中都改为不崩溃路径（先判等，必要时使用地址字符串排序；hash 使用固定标签）。
- FFI 注入层保持错误可恢复语义，避免 `expect/unwrap/unreachable` 直接中断。

## 关键文件

- `src/calcit.rs`
- `src/calcit/record.rs`
- `src/program/entry_book.rs`
- `src/program.rs`
- `src/calcit/local.rs`
- `src/bin/injection/mod.rs`

## 测试与验证

- `cargo test` 通过。
- `yarn check-all` 通过。

## 经验记录

- 对核心数据结构相关路径优先采用“可恢复失败 + 明确错误信息”，比 panic 更利于线上可观测性。
- 涉及全局索引缓存的数据结构（如 `EntryBook`）应优先保证越界/墓碑状态可检测并可回退。
- 对 AnyRef 这类 FFI 相关动态值，优先保证运行稳定，再在文档中标注语义边界。
