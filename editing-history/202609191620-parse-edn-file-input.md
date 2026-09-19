# 为 parse-edn 增加文件与 stdin 输入

- `calcit cirru parse-edn` 保留内联参数兼容性，并增加 `--file <path>`。
- `--file -` 显式读取 stdin，省略输入则快速报错，不会隐式等待管道。
- 内联内容与 `--file` 同时出现时拒绝执行，避免输入来源不明确。
- 集成测试覆盖超过 Linux 单参数常见上限的大文件、stdin、兼容内联路径与错误边界。
