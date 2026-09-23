# JS 生成代码去重命名空间导入

关联 #1323。Cumulo Reel 的 server 入口可完成 JS 生成，却在 Node 语法检查时失败：同一生成文件重复声明 `$cumulo_reel_DOT_schema`。最小复现包含同一命名空间的 `:as` 值引用与 `parse-cirru-edn-as` 名义类型引用。两条来源分别将 `NsAs` 记录到导入集合，其别名不同；最终输出却都使用真实命名空间生成同一个 JS 绑定。

JS emitter 在输出 `NsAs` 语句时按真实命名空间再去重，保留普通 `:refer` 等不同导入。最小项目的严格检查和生成通过，生成文件只有一个命名空间绑定，Node 语法检查与实际调用 `main!` 通过。CLI 回归每次通过 Calcit 编辑命令构造 Snapshot，不提交手改的程序文件，并验证值引用和名义解码器引用都仍存在。

隔离的 Cumulo Reel 副本重新生成 server JS 后，原先报错文件的 Node 语法检查通过。安装该副本原有 npm 依赖后尝试加载 server 模块，已越过重复导入错误，但停在旧 `calcit.std` 的 `path-exists?` 所生成的 `$clt._$n_call_dylib_edn is not a function`；同时运行时 `@calcit/procs` 仍为消费者声明的 0.19.1，和本机 0.20.0 编译器不匹配。完整消费者业务运行应作为依赖升级后的单独验收，不能以单文件语法检查替代。
