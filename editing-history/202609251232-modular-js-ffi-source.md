# 模块内 JS 实现来源

0.22 的 JS FFI 采用定义级 `:ffi :js`，其 inline 与 file 形式共用现有 Fn schema 和词法 `:js-ffi` 权限。调用端仍通过普通 Calcit namespace 导入。实现内容不混入 Calcit AST，也不使用 eval；生成时转为私有 ESM 资产。

文件路径以声明所属模块的 Snapshot 目录为根，而非消费者 cwd。loader 保留 namespace 到模块身份与源目录的映射；输出使用模块身份和相对资源路径，处理明确列出的本地依赖并保留 ESM 单实例。路径及 symlink 越界在复制前拒绝。schema 仅是作者提供的契约，因此第一版限制在直接对应的基础 ABI 类型。

目前的外部消费者 fixture 验证源码模块、file helper 和 inline JS。发布前仍需检查已发布依赖的真实安装、跨模块菱形依赖、缺失导出、增量删除及更完整的错误来源映射；这些边界不可仅凭生成成功视为完成。
