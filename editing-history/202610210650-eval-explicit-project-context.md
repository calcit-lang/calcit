# eval 保留显式项目上下文

## 问题与选择

显式 `eval --dep` 曾复用项目依赖合并规则，将默认 package `app` 下的 namespace 全部过滤。直接取消过滤仍不够：先创建的片段 `app.main` 会覆盖应用入口，或者因同 package 的优先策略静默吞掉真实冲突。

eval 在安装片段前尚不拥有项目 package，因此先以无 package 所有权的临时 Snapshot 严格合并全部显式依赖，保留已有 JS FFI source ownership 检查；依赖配置的 init/reload 不会被选作 eval 入口。之后为片段选取未占用的 namespace，保持无冲突时原 `app.main`，冲突时逐个避让已有名称，再把片段的 ns 和默认 entry 同步指向该位置。没有新增用户 flag 或另一套依赖解析器，也不改变普通项目依赖的 cycle/authoritative-source 语义。

重复依赖的目录/文件相对写法原先通过，但同一 Snapshot 再写绝对路径时，生成的 `$meta` 保留不同路径字符串，严格合并会错误拒绝。eval 现在只在本次命令内复用“实际模块目录 + Snapshot 文件名”相同的直接依赖，保留首个加载结果和来源。只规范化父目录，不解引用 Snapshot 文件本身：两个资源目录只链接同一个文件时仍是独立来源；不同文件名也不合并。无法取得目录身份时仍由原 loader 报错，不修改其诊断、真实 namespace 冲突或 JS 资源 owner 检查。

## 验证范围

复用 `cli_stdout` 集成测试，复制既有真实 app fixture，通过 dry-run、scoped revision 事务使项目 init/reload 明确抛错，并制造第一个内部候选名称已占用的场景。文件、目录、绝对路径、等价重复依赖、Unix 目录软链接和首行 require 均应读取原 app.main 的 Number 定义并通过原语言断言；只软链接文件的独立目录及不同文件名仍报告实际 namespace 冲突。显式调用仍传播错误，错误参数必须在预处理阶段以具体诊断拒绝，源文件保持不变。现有 util 项目上下文及 JS FFI owner 验收继续保留。

此处的进程/加载边界不能由 definition :tests 选择 eval 的上下文，因此留在现有 CLI 测试中；值与类型仍直接来自 Calcit fixture，不复制语言实现。最终结果、review 与精确 main 记录在 PR，不以本记录声明已验收或已发布。
