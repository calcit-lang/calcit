# 2026-07-21 01:10 Cirru 格式化与文档命令更新

## 修改概要

- 使用 `cr <snapshot> edit format` 统一格式化 `calcit/` 下 56 个非空 Cirru 文件。
- 保留 `calcit/debug/.compact-inc.cirru` 这个 0 字节热更新占位文件原样。
- 修正文档中已移除的 `check-md -d`、`remote-libs readme -f`、`read-lines -s/-n` 旧参数。
- 在快速参考中加入文档知识图谱常用命令。

## 验证

- `cr docs agents --full`
- `cargo test -q`
- `yarn check-all`
- `git diff --check`
