# 谓词收窄使用真实来源

## 问题与决定

真实 Respo 迁移调查中，需要区分普通 `Bool` validator 与编译器能证明的类型判别。最小反例在用户 namespace 保存同名 `string?`，使用合法匿名 Fn initializer 与 `Fn(Dynamic) -> Bool` schema，函数恒返回 true。先前只按 Import/Symbol 短名收窄，导致 `count value` 错误通过严格检查并被 lowering 成 String 计数；输入 Number 时才在运行时失败。

复用原分支收窄入口，仅真实 builtin proc 或 core 来源的既有谓词合同授予证据；用户函数仍可普通调用，未解析 Symbol 不授予证据。没有新增诊断、谓词注册表、type-guard 语法或固定迁移规则。

正例另暴露限定 core primitive 会误编译 metadata 占位符。直接 core 和 namespace alias 的 primitive 引用现在通过一个公共解析函数得到原 runtime proc，普通 core Fn 仍走原 source 路径；其他 namespace 同名定义不转为 builtin。

## 验证与范围

沿现有 count-contract fixture 追加两组 definition `:tests`，验证直接 core 引用、namespace alias、Unicode 字符串与非字符串分支；原五组测试和方法/函数表达式保持不变。既有运行器回放 native/JS，并构造六类同名用户谓词，在 check/native/JS/WASM/WASI 的预处理阶段拒绝 Countable 假证明，保持 Snapshot 字节不变且不生成应用/WASM 产物；保留 CLI 的空输出目录与含原诊断的 JS build-errors module。内部测试覆盖可信 Import、伪造 ImportInfo、未解析 Symbol 以及 nullable core 别名的正反收窄来源。

本修复不自动推断任意用户 validator，也不把普通 Bool 签名当作名义 decoder；Respo 的 Component 边界和其他真实迁移仍需继续处理。关联 #1728、#1729、#1694、#1529 与 #1553。

审查进一步确认 CLI 会保留项目提供的 `calcit.core`，所以 Import 的 namespace 不是可信来源。源代码谓词还需核对实际 embedded core 的 import、全部定义代码与 FFI 数据，避免不变的包装函数依赖被替换的同名谓词；doc/examples 的改动不改变执行身份，重新加载得到的名义 schema ID 也不作来源身份。期待值从 embedded source 生成并缓存，不维护第二份谓词注册表。实际 proc 身份保持可信，项目自定义 core 的 nullable 谓词正反分支由现有 runner 验证拒绝。
