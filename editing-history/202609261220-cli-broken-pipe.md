# #1318：文档与查询的 stdout 提前关闭

公开 0.22.0 与 #1378 合并后的 main 仍能通过以下命令复现退出码 101：

```sh
zsh -o pipefail -c 'calcit docs search Fn | head -n 1'
```

Rust 的标准打印宏遇到 BrokenPipe 会 panic。此次只让 docs/query 和已有共享
Markdown、提示渲染调用小型 stdout helper，不重写返回类型、引入输出框架、
添加命令、安装全局 panic hook 或改变进程信号处理。

helper 持有 stdout lock 完成一次格式化写入并显式 flush，确保短输出或末尾输出
的失败不会留到进程关闭时才发现。BrokenPipe 表示读者主动结束，退出 0；其他
输出错误写 stderr 并退出 1。运行时效果和 FFI 的输出保持不变。

测试放在 Rust CLI 集成层：真正的子进程与提前关闭的管道覆盖 docs search、
完整指南、Markdown 读取以及 human/EDN/JSON query；另测读首行后关闭。
Linux 用 `/dev/full` 验证非 BrokenPipe 失败不能变成成功。不能用只读文件描述符
替代这个测试：Rust stdout 会特殊处理部分无效描述符，不能可靠产生预期写入错误。
正常输出继续由既有 Markdown/结构化 CLI 测试与 Agent interface 验证。

文档明确：提前关闭的成功状态不是截断后 JSON/EDN 可解析的保证。
