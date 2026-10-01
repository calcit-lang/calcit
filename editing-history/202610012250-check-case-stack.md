# 大分支表达式的预处理栈占用

## 问题与证据

#1584 的 macOS HN Reader 普通检查在进入 Markdown 的 `operator-command` 后 abort。系统崩溃报告的 UUID 与本机缓存的 0.27.0 Mach-O 二进制一致，显示 32 MiB 主线程栈耗尽、预处理表达式递归深度约 342，栈顶正在执行宏。当前依赖与原 issue 的依赖已不完全相同，不能把当前 keep-going 的 4 个类型失败描述为原来的全绿结果。

最小复现没有外部模块：20 层普通函数依赖，末端使用 160 对 String/Number `case-default` 分支。修复前当前未发布构建与缓存 0.27.0 macOS 构建均以 134 退出。公开源码版本、发布产物及本机缓存构建要区分：官方 0.27.0 的 `calcit` release 资产为 Linux x86_64，不能用它证明 macOS 二进制来源。

## 实现边界

把已知函数调用的类型检查分支抽为独立且不内联的函数，复用现有 `PreprocessContext`。宏、syntax 与其他调用不再为没有执行的函数检查分支预留临时变量栈空间。保留参数顺序、泛型关系、期望 callback 类型、schema 检查、类型提示清理与已有 lowering；不增加 `case-default` 特判、不扩大主线程栈、不新增依赖、命令、诊断或 Dynamic 例外。

旧缓存构建中 `preprocess_list_call` 的汇编栈分配为 `0x12880` 字节；拆分后的当前构建为 `0xfe30` 字节。这是不同源码版本的辅助证据，不能作为严格同 revision 性能基准。直接修复前后执行同一个当前构建最小复现，分别得到栈溢出与检查成功，才是行为回归证据。

## 验证

夹具由 Calcit edit/schema/add-test 创建，并从临时项目复制到 `tests/fixtures/large-case-check.cirru`，未手工修改 Snapshot 文本。语义保存在 `lookup` 的 definition `:tests`，检查首、中、尾、默认分支；Rust 测试负责普通与 keep-going CLI 子进程不 abort，并在最后一个分支加入错误的 Number 参数，要求类型检查正常失败而非崩溃。

复现命令：

```sh
calcit tests/fixtures/large-case-check.cirru --check-only
calcit tests/fixtures/large-case-check.cirru --check-only --keep-going --format edn
calcit tests/fixtures/large-case-check.cirru test app.main/lookup --require-match
CARGO_INCREMENTAL=0 cargo test --test strict_check_cli large_case_under_a_dependency_chain_checks_without_aborting
```

这减少具体递归路径的栈占用，不承诺任意深度表达式或宏不会耗尽有限资源。真实 HN Reader 的原始依赖固定与其他消费者回放仍是 #1584 后续验收，不能只凭本夹具提前关闭整个 issue。
