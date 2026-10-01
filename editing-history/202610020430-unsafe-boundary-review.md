# unsafe-coerce 边界的待审导航

## 决策

沿 #1539 复用严格预处理已经产生的 `E_UNSCOPED_UNSAFE_COERCE`，不增加类型关系、统计门禁或新的 CLI 入口。
显式 `unsafe-coerce-boundary-v1` 在现有 fix suggestion 中保存 compiler diagnostic、源码位置、指纹与编译栈。
最近的源码函数路径只是导航上下文，权限是否满足仍由编译器决定；macro 展开的编译栈不能用源码文字匹配代替。

此规则不进入默认 preset，不使用会关闭 strict-only 边界错误的 legacy migration mode。遇到边界错误时输出
可解析的计划，但保留非零退出和 error severity；replacement 与 operation 始终为空。preview、apply、重复调用
与 revision 失配均不改原始源码，也不添加 `:js-ffi` 或选择业务 decoder。

预处理在每个 definition 的首个错误停止，因此公开说明结果不是穷尽扫描。依赖或 scope 外的错误须重新选择真实
源码 owner；没有位置或不相关错误不转成空的成功计划。`:tests` / `:examples` 不在本规则检查范围，仍独立回归。

## 验证

低层 CLI 回归检查协议、非零退出、原子 no-write、revision、独立 definition 权限隔离、合法 adapter 和无关错误。
合法 adapter 的值语义通过附着的 Calcit `:tests` 验证；此次不修改语言或 backend coercion 语义，复用全量 native、
JS 与现有 WASM/WASI 门禁验证不变性。不为等待上游列表依赖发布而使用本地包修改正式依赖。
