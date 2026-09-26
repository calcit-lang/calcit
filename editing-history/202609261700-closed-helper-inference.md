# 闭合零参数 helper 的函数体推断

关联 #1307；前置 #1384 已保证缺省 schema 在编辑后不会变成显式 Dynamic。本步选择 Respo 的 `respo.resource/next-resource-id!` 作为实际案例：函数体从已知 Ref<Number> 读取并递增，重复的 root Fn () -> Number 可以省略。

只处理普通、非递归、零参数且没有 FFI 元数据的缺省 defn。复用现有函数体预处理和 callback 签名推断；完整闭合才注入内部 hint-fn。显式 Dynamic、参数未证明、递归、WASM 导入导出与异步边界保留原检查。没有从调用样本推断输入，也不写回 Snapshot。递归检测复用编译依赖与当前预处理中的定义集合。

源函数在 compiled cache 中通常保存 defn 而非运行时 Fn。调用返回类型读取其中的已检查签名，使下游普通方法能够继续静态 lowering，而不是改成 native call。声明过的公共契约仍然优先。

context 将 inferred-schema 与源码 schema 分开，并复用同一预处理结果；type-at 在源码 root 上显示同一函数类型。无可信预处理结果时保留错误/未解析诊断，不把源码表达式猜测当作已证明契约。

语义测试写在 calcit/test-helper-inference.cirru 的 :tests。同一 AST 由既有 check-js-runtime 流程运行 native、JS、WASM。脚本另验证查询只读、一致证据、错误使用、开放容器、冲突分支、直接/相互递归、未证明参数和显式边界；编译失败属于 CLI 协议测试，不伪装成 runtime try。

本步不关闭 #1307：有参数/泛型 helper、真实应用整体迁移和发布包回归仍需后续小步验证。具体命令及结果以 PR 和 issue 为准。
