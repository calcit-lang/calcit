# 文件读取 primitive 与模块名称边界

关联 #1570、#1457、#1458。std 去掉读取 API 的效果后缀时，`read-file` 与 `read-dir` 被 reader 提前解析为 Proc，受控 rename 无法声明同名模块函数。

底层改用 `&read-file`、`&read-dir`，core 原有名字指向同一 Proc 值，而不是增加一层变参 wrapper；保留原来的 arity、String/List<String>、目录递归默认值和错误模型。JS codegen 映射到既有 read_file/read_dir runtime exports 和 host injections，不要求宿主改名。公共符号现在能携带普通 source location；局部同名绑定仍受已有严格 shadowing 诊断约束，不借修复扩大名字覆盖权限。

文件行为写在 Calcit attached tests，native 与生成 JS 运行同一测试表达式；脚本只断言宿主调用次数/参数与 shadowing 诊断。Rust 覆盖 primitive 注册、JS export 映射和原子 rename 事务；后者用无宏的失败断言保持现有 rename 对宏 source 的拒绝边界，不为让测试通过增加宏白名单。std 实际 Snapshot 的两项 rename 预览已通过 staged 检查，正式迁移仍等待 release-matched 依赖，不修改 Rust ABI 或文件能力。

额外以两个独立 Snapshot 验证依赖模块与 consumer：消费者通过正常 `:as` 引用调用模块的 `read-file/read-dir`，共享两项 Calcit `:tests` 在 native/JS 验证返回值，并检查没有误走 core 文件宿主注入。模块与消费者使用不同 package，符合现有项目命名空间归属规则，不修改 loader 或 resolver。
