# 被内建名称掩盖的无效导入

`generate-id!` 等裸内建名优先于 namespace 的 `:refer`。真实消费者曾声明不存在的
`respo.util/generate-id!`，仍能通过严格检查并运行内建函数，导致维护者无法从检查结果发现错误来源。

## 决策与范围

在现有预处理的 Proc / Symbol 解析分支复用同一检查：只有被实际预处理的内建函数存在
同名显式 refer、目标不是 core / JavaScript namespace / 已注册宿主过程，且 Calcit
源码中不存在该定义时，发出已有的预处理警告。不新增命令、诊断编号、扫描器或固定改写规则。

源码读取器已将内建名称转为没有 token 坐标的 Proc；不能只修改 Symbol 分支，否则最小复现仍然通过。
Proc 分支用现有编译上下文定位所属定义，Symbol 分支保留原坐标。只有真正发现无效导入时才构造位置。
警告沿用严格检查与 JS 生成的既有阻塞规则；这属于原定 0.32.0 范围，不作为 0.31 发布线的补丁合并。

内建选择顺序、ID 算法、有效 import 的行为不变。存在性检查不调用被遮蔽定义的预处理或求值，
不强迫按需加载的实现提前通过检查。未使用 import、quote 中的内建名称、JavaScript exports
的真实性仍不由这项诊断审计；宿主合同继续由已有 FFI 边界负责。

## 验证方式

- 正常行为写在已有 typed-source-alias fixture 的 definition `:tests`，验证直接内建、core refer、
  有效模块 refer 的内建优先级及完整限定名的模块实现，并使用已有运行器回放 native / JS。
- 无效 namespace、存在 namespace 中缺失的 definition、内建一等函数引用、JS 生成拒绝和限定名不回退，
  扩展已有 source-alias 宿主脚本；这些程序应在执行前失败，不能伪装为运行时 attached tests。
- quote 不解释内建引用，JavaScript namespace 不假装是 Calcit 源码；被遮蔽实现故意放入类型错误，
  验证只选择内建时不编译它，而显式检查该实现仍会失败。
- 测试只通过官方事务修改临时 Snapshot，先 dry-run，再传 scoped revision；永久 fixture 也通过同一入口修改。
- 同一最终用例在未修复 CLI 上必须失败，在候选 CLI 上通过；保留既有 alias、循环与 lazy-producer 回归。

具体提交、命令和 CI 证据记录在关联 PR，不以这份历史记录替代用户文档或可执行验收。
