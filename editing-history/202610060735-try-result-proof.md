# `try` 的独立返回证明

Diary 的严格升级遇到 `js-ffi.browser/local-storage-available?` 与 `calcit.core/try-parse-cirru-edn` 返回类型未知。最小用例中，正常分支和有明确签名的错误处理器都返回 Bool，运行正常，但 strict workflow 仍要求人工补充证明。

根因是已预处理的 `try` 没有返回类型推断入口。修复为错误处理器提供运行时已有的 String 参数上下文，再复用普通 callable 检查及 if 的类型分支合流规则。处理器返回类型不从外层声明反推，不新增诊断、改写规则、检查器或权限。

源码保留 `try` 的原始 body 与 lazy handler；证明使用的 String 调用不进入执行树。正常 raise 分支不产生值；处理器的 Never 返回使用现有底类型规则。预处理函数的 callable 类型还保留实际参数布局，避免上下文或 hint 为零参数函数凭空增加参数。开放或矛盾的返回、错误参数和缺少 FFI 权限仍应失败。

语义契约写在 `calcit/test-struct.cirru` 定义的 `:tests`，由现有 `scripts/check-known-assertion.mjs` 回放 native/JS，另在无依赖临时 Snapshot 验证完整 strict workflow，检查失败不会改变源码或生成应用产物。未增加 WASM try 能力；原 WASM 用例继续验证现有边界。#1553 落地后迁入统一改写后回归集。

这只是 Diary 剩余诊断中的一类修复；模块字段契约、其他 core 泛型与业务状态边界仍按各自真实证据验收，不能据此宣称整个迁移完成。
