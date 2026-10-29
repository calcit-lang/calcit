# keep-going 与 all-defs 报告建议级诊断

#2124 的生态普查需要 `--check-only --all-defs` 覆盖项目全部定义，但该路径只返回 definition 的 warning 与 error，预处理期间收集的建议级诊断（`W_OPTION_ERASED_TO_NIL`、`W_IMPLICIT_NIL_BRANCH` 等）从未取出，human 与结构化报告都看不到。普通 `--check-only` 只覆盖入口可达闭包，因此 Respo main 在入口范围只显示 2 处 Option→nil，全部定义范围实际有 6 处。

strict check 在第一轮检查后取出建议级诊断，证明审计重新预处理时产生的重复项照普通检查的做法丢弃。human 输出在 stderr 以与普通检查相同的 `Notes:` 格式列出；EDN/JSON envelope 增加 `data.advisories`，项的形状与 definition 诊断一致，`severity` 为 `note`。建议级诊断不改变 definition 状态与退出码。

CLI 回归覆盖 all-defs 下未被入口引用的定义产生的 note（JSON 与 human），同一报告中另有失败定义时 note 仍保留，以及入口可达 keep-going 报告为空数组。
