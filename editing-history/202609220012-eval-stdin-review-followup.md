# `eval --stdin` review 补充

CodeRabbit 指出多行位置参数示例与 stdin 推荐文字容易产生歧义；文档现明确区分“仍支持多行位置参数”和“遇到 Shell 转义或管道输入时推荐 stdin”。CLI 冒烟测试把错误退出码收紧为 `1`，并使用仓库内已有的两个 Snapshot 作为重复 `--dep` 测试，验证 stdin 求值能实际调用两个依赖中的定义。
