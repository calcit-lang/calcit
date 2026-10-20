# 内部 Map / Set 解构保留名义成员关系

关联 #2187。维护者明确批准 0.31.0 的 `&map:destruct` / `&set:destruct` 直接返回已有
`MapDestruct<K,V>` / `SetDestruct<T>`；公开 `destruct-map` / `destruct-set` 和 0.30.1
维护线不变。

原开放 `Option<List<Dynamic>>` 需要包装函数按索引恢复互不相同的字段，无法独立证明其泛型关系。
新实现直接构造独立 payload，不添加新的结果类型、异质 List 语法、断言、unsafe 或证明豁免。
native 与 JS 复用同一 core 名义定义，失效/重绑定策略与已有 Option 路径共用；预处理根据原语
结果合同保留名义定义依赖，避免只传递结果的 JS 程序漏掉初始化。WASM 沿用现有 enum 内存布局，
省去中间 List；集合选择、剩余集合计算与错误路径保持原实现。

原定义测试保留空与非空分支、Map fold 断言，并补充字段类型、nil/false/0/空字符串、Option payload、
原输入不变与剩余集合。共享运行器负责 native/JS/已有 WASM 的真实回放；宿主 registry 身份与重绑定
属于 JS runtime 低层 invariant。升级说明要求审阅直接调用、附带区域与宏消费链，通过现有 guarded
transaction 迁移，不新增固定改写矩阵。完整 kind→T 返回/赋值证明仍由 #2187 后续修复，不以本切片代替。
