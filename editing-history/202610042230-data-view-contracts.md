# Data 分类与浅层类型证明

已有 PR #1765 引入 Data/data-view，为开放值提供 nominal match 分派。收尾检查发现 value-only 的 enum?/struct? 对类型定义会抛错，导致 :other 不能接收 EnumDef/StructDef。改为先识别类型定义，再沿现有谓词分类；静态构造使用 Data/Option 直接构造器。

Data 的标量 payload 使用具体类型，集合保留一层容器形状；元素和 fn/enum/struct/ref/other payload 仍是 Dynamic。不会承诺 deep decode、零分配、废弃 type-of 或未验证的 WASM 支持。

四个 definition :tests 覆盖十四个分类、false、Symbol/Struct/Ref、混合集合原值、类型定义和无 wildcard 的完整 match。扩展既有 check-known-assertion 运行器回放原 AST 的 native/JS，拒绝错误标量、开放元素进入具体 Struct 字段、非穷尽 match，检查失败不写 Snapshot 或输出应用模块。关联 #1766。

额外发现发布 alpha.3 中，List<Dynamic> 元素进入具体局部 Fn 参数仍可通过严格预处理，直到算术运行时失败；候选 Data 的具体 Enum payload 也接受该开放元素。这些缺口由 #1767 追踪，不扩大本次 API PR 的编译器改动，不将它们记录为通过的负例。
