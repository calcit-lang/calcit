# 有限标量的文本转换契约

## 决策

在 #1456 中增加 `ToString/.to-string` 和推荐入口 `to-string`，沿用已有 trait 推导，不增加逐函数的检测规则。只有 Nil、Bool、String、Tag、Symbol、Number 配置内建实现；Unit、CirruQuote、集合与未实现的名义类型不获得转换能力。`turn-string` 与 `turn-str` 暂保留历史兼容，不批量改写调用。

`Dynamic` 和 `DynFn` 不能在泛型 `where` trait-bound 的使用点直接充当实现证明；其他类型仍沿用既有兼容关系。这样开放值仍可保存、传递和包装，调用具体转换时才需要解码或显式的类型证据。Respo 的 `DomElement` 宿主边界依赖现有名义兼容关系，不在本次转换任务中重写其类型解析。

## 后端与迁移边界

native 与生成的 JS 对六类标量的转换复用现有 `turn-string` 语义；Number 小数、负零和非有限值使用此前已对齐的格式。现有 core WASM 对 `to-string` 的 trait 依赖会明确报告 unsupported 并生成陷阱，不能默默返回错误文本；旧 `turn-string` 对运行时小数仍有占位输出。WASM 的完整文本转换是 #1456 后续工作，不因本次静态契约落地而关闭。

`to-string` 不是 Debug、Show 或序列化入口。旧 `turn-*` 调用只有在输入类型已证明、输出和失败语义等价、source AST 可唯一定位且有 revision/fingerprint 前置条件时才能进入 guarded fix；本批不新增自动改写。

## 验证

- Calcit core definition `:tests` 验证六类标量、返回类型与泛型 bound；严格负例覆盖集合、Unit、FsPath、开放 JSON 值。
- native/生成 JS 对照脚本验证转换结果；WASM 集成脚本验证显式 unsupported 陷阱。
- 全量 `yarn check-all`、Rust 测试和 Clippy；在真实 Respo Snapshot 上执行 `js --check-only`，防止泛型边界过度收紧。
