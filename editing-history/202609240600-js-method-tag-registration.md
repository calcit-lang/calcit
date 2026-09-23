# JS 方法式 tag 访问的注册边界

关联 #1313。`reel.:store` 在 JS 输出中会生成 `reel.get(_t_.store)`，但此前只有源码中的显式 `:store` 值会进入当前 namespace 的 tag 表。若 Map 在另一个 namespace 构造，编译与静态检查都可能成功，而当前模块的 `_t_.store` 为 `undefined`，运行结果错误。生成模块不能依赖提供 Map 的模块替调用方注册 tag。

JS emitter 现在在生成 `MethodKind::TagAccess` 时，把方法名也加入当前 namespace 的 tag 集合；其余代码生成和类型规则保持不变。回归使用 `util.core` 构造带 `:store` 的 Map，`test-js.main` 只以方法式语法访问它，不在该模块写显式 `:store` 值。definition `:tests` 固定 Calcit 可见语义，`yarn try-js` 实际执行生成的 JS。修复前，JS 测试把预期 `1` 读成 `nil`；修复后当前模块的 tag 表含 `store` 且运行通过。

当前严格类型规则对普通 Map 的前缀 `(:store reel)` 给出 Struct 字段诊断，因此这里不为了表面上的前缀写法对照而加入 Dynamic 或兼容开关。Map 的方法式访问与跨模块 tag 注册是本次修复范围；前缀字段访问应继续遵守现有静态类型契约。
