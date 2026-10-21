# JS FFI 编辑先校验再写入

#2219 的真实消费者报告事务 apply 返回错误，但源码已经改变。最新 main 的隔离
CLI 回归还复现出普通 `edit ffi` 会接受缺少实现来源的 `:ffi :js`。现有合同校验
在 program 构造阶段执行，编辑 writer 没有调用它；存在 cursor 等后续维护时，
错误才可能在原文件写入以后暴露。

## 决策

复用现有 `validate_js_source` 与 program 的 `defn` 约束，合成共享的 CodeEntry
校验，不增加 JavaScript parser、checker、错误编号或宽松 loader。program 继续
使用同一合同；Snapshot render、保留原 EDN 的单定义 schema/ffi writer 在写盘前
调用它。事务的子操作和最终序列化因而也在提交原文件前拒绝错误元数据。

检查保持原范围：inline/file 来源、资源路径、显式 target、module specifier、
Fn、`:js-ffi`、既有 ABI 类型与声明头。不解析、读取或执行外部 JavaScript。
保持原有查询/修复加载流程，不把这些检查追加到读取 loader；schema writer 继续保留
未选中源码合同的原形式。
不做新的函数体证明，不让实现片段的声明签名代替外部运行时测试。

先准备完整 Fn 和 feature，再设置完整 JS 实现；需要清除 schema 时，先清除该
JS 实现元数据。原 guarded transaction、scoped revision、writer lock、source
格式与外部文件边界均保留。

## 验证方式

此问题是 CLI/文件系统回滚 invariant，新增回归留在既有 Rust `edit_cli` 集成
测试，未用 Rust 复制语言语义测试，也未新增检查脚本。

- 修改前新增回归在普通非法 `edit ffi` 未被拒绝处失败。
- 修改后直接编辑、dry-run 和带 scoped revision 的 apply 对缺来源、缺 target、
  越界 file 路径、browser 导入 node 模块均拒绝，原 Snapshot bytes 保持不变；
  已成功暂存的另一个定义 doc 也不能提交到原文件。
- 有效 inline 事务仍可使用原 scoped revision 提交。随后不兼容的 schema、清除
  必需 Fn、把 `defn` 改成 `def` 均不能改变原文件，cursor 保持可用。
- 合法 file 元数据无需读取不存在的外部文件即可保存；清除实现后，原 schema
  清除流程仍然可用。
- 原编辑 CLI 40/40、Snapshot 84/84、JS FFI 跨模块/来源映射/宿主合同运行器和
  Agent interface 74/74（另有 view reading 8/8）通过。原 JS source validator 4/4、
  `cargo clippy --all-targets -- -D warnings`、格式化、whitespace 与指定 Snapshot 的
  JS interop Markdown 检查通过。没有把整套 CI 的未运行部分记作本地通过。
- 真实 Respo 的隔离 Snapshot 副本中，同样验证直接/dry-run/apply 拒绝非法 metadata
  且 bytes 不变、完整 metadata 可提交；canonical source 未变。使用本地未发布候选，
  不代替正式 CLI/runtime 配对或生态发布验收。

没有放宽 Dynamic、ABI、target、feature 或 Fn 合同；完整 CI、实际 review、评论
处理及精确 main 验证仍是合并/发布门槛。
