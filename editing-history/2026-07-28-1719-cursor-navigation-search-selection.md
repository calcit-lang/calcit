# Cursor 多步导航与搜索选取

## 概要

- `cursor child` 省略 index 时进入首子节点，`cursor child --last` 按当前树进入末子节点。
- `cursor next`、`cursor prev` 与 `cursor back` 支持 `--count N`；多步移动只产生一次 history transition，参数为零或越界时不写 cursor。
- `query search` 与 `query search-expr` 的每个结果获得稳定的全局 cursor index：human 输出为 `[#N]`，JSON 字段为 `cursor_index`。
- 搜索命令新增 `--set-cursor N`，直接将对应结果写入项目 cursor。确认信息走 stderr，保持 JSON stdout 为单个可解析对象。
- dependency-only 搜索结果不能映射到当前可编辑 snapshot 时明确报错，不将 cursor 猜测性落到同名定义。

## 验证关注点

- `child --last` 必须读取当前 node 的 child count，不能把 `-1` 或陈旧 index 写入 sidecar。
- sibling skip 和 multi-back 必须预先验证完整步数；失败后 active cursor 与 history 均保持不变。
- human 与 JSON 的结果编号必须共享同一排序和扁平化顺序。

## 验证结果

- `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test`、`yarn compile` 与 `yarn check-all` 通过。
- Agent interface smoke 12/12，`docs/run/edit-tree.md` 与 `docs/CalcitAgent.md` 的可执行代码块检查通过。
- 全局安装 `cr 0.12.53` 后，在 `respo-calcit-workflow` 临时副本以 `query search --set-cursor 0` 定位 `comp-runs`，随后验证 `child --last`、`prev --count 1`、`back --count 2`、focus 展示，并完成 `cr calcit.cirru js` 编译。
