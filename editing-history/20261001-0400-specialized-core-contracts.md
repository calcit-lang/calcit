# 补充具体调用类型证据和已审阅家族

#1568 的首批基线只保存声明 schema 和 proven 状态；对 `get` 这类宽实现来说，二者不足以冻结具体 List/Map 调用的结果。现有 query type/context 复用 StaticMethodContract 导出类型语法节点，不增加公开命令、开关、运行时规则或 registry，默认 human 输出不变。

原生 EDN 基线新增 method-contracts，保留具体 receiver 的参数、rest、返回类型和 callback 泛型关系。旧方法声明和失败记录不变，历史基线比较只允许新增证据，不借此改写既有契约。query 原有显示字符串保持兼容；基线不拿显示文本当作唯一类型证据。

扩大已审阅范围到谓词、集合首选入口、公开数学函数与 ToString/Len/Add/Eq、FsPath 首选构造。不按 internal tag 一刀切，也不把尚为 open 的 Option.map 或开放 parse 伪装为类型证明。继续保留 #1568 的全量审阅、alias 排期、backend 矩阵、集中迁移和发布消费者验收。

新 Rust 测试仅验证 CLI 类型节点序列化边界；语言语义仍复用原有 Calcit attached tests。工具负例额外覆盖源 schema 不变但具体调用类型或 callback 关系变化的情况。

验证：cargo fmt --check、cargo clippy -- -D warnings、完整 cargo test、yarn check-all（包含 compile、native/真实 JS/WASM 与 attached tests）、50 项 Agent interface、17 项契约工具测试均通过。原生 EDN 输出核对 List.get 的 Option<Number> 节点和 open 方法缺字段；query 文档可执行片段 1/1 通过。已全局安装的正式 Calcit 0.27.0 在已迁移的 Respo 副本严格检查通过；不把它冒充未发布 0.28 的全量生态验收。
