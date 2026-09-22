# 收紧存量 quality baseline 写入边界

`analyze quality --write-baseline` 过去可创建新文件或提高已有预算，与“仅供存量项目清债”的文档定位不一致。现在要求目标文件已存在，且当前指标不能超过该文件的预算；原生 baseline 按 definition 比较，扁平 baseline 沿用汇总指标比较。v1/扁平文件升级写出 v2 时，原先未约束的 `unsafeCoerce` 不得凭空获得非零预算。校验在既有原子写入的 writer lock 持有期间进行，直到替换完成才释放，避免两个 CLI 写入者在校验与提交之间相互覆盖。

普通 `--baseline` 读取与比较旧文件的行为不变，默认严格类型诊断仍是正确性来源。新项目不应建立统计 baseline，已有项目清零后删除该命令和文件。
