# 自尾调用改写后的 recur 参数检查

预处理把处于尾位置的直接自调用改写成 `recur`。改写发生在 `recur` 参数检查之后，而改写前的自调用因为函数仍在编译中，也不按签名检查参数。结果是参数顺序写错的自尾调用（如 `typed-recur (inc index) label`）通过 `--check-only`，运行时才失败。开启 `CALCIT_LINT_CORE=1` 的改写后校验在最终节点树上报出了这一漏检（#1553）。

改写函数现在返回它新建的 `recur` 调用，预处理对这些调用运行与手写 `recur` 相同的参数检查，沿用 `W_RECUR_ARG_TYPE_MISMATCH` 与调用位置，不新增诊断编号。`calcit.core` 与带标记参数的函数沿用手写 `recur` 的既有跳过规则。原有 `recur` 节点不会被重复检查。

验证：`tests/strict_check_cli.rs` 的 recur 用例补充自调用的正确与错误顺序；完整 `cargo test`、开启校验的 `yarn check-all`，以及 `~/.config/calcit/modules` 下各模块 `--check-only` 与已安装版本的结果对照（无差异）。
