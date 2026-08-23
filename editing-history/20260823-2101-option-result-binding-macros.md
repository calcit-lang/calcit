# Option / Result 顺序绑定宏

- 放弃 labelled arguments 方向，避免参数声明、函数类型、偏函数和 codegen 同时增加语法与绑定规则。
- 在 core 中实验性增加 `option:let` / `result:let`，复用普通 `let` binding pair，并完全展开为接收者 `.and-then` 调用。
- 宏不自动添加 `%some` / `%ok`，不提供外围函数 early return，也不在 Option、Result、nil 与 JsNullish 之间隐式转换。
- 方法调用预处理现在会先用 receiver 绑定泛型，再把专门化后的 Fn 参数契约注入匿名 callback；callback 的返回类型因此能检查容器外层和 Result 错误类型。
- 泛型函数返回检查不再因包含 type variable 而整体跳过，而是允许绑定 payload，同时继续验证 `Option` / `Result` 等外层结构。
- 普通 Option/Result 能力保持 method-first：公开代码优先 `.map`、`.and-then`、`.or-else`、`.map-err`、`.unwrap-or`；命名空间函数主要服务 core lowering。
- type-fail Snapshot 恢复普通文本 diff，不再通过目录级 `.gitattributes` 隐藏 Calcit 源码。
