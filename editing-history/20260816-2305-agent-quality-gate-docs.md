# 2026-08-16 23:05 Agent quality gate 文档澄清

- 明确 Agent 首次建立原生 quality baseline、人工审阅后提交，以及 CI 使用 `--baseline` 比较的完整路径。
- 说明 `analyze quality` 的非零退出码就是发布回归信号，JSON 输出仅用于机器读取和定位。
- 修正升级与类库质量文档中旧的“拼接 JSON 比较”表述，保留各分析命令作为定位报告。
