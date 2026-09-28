# List 文本拼接命名收敛

关联 #1455。现有 `join` / `.join` 返回 List，语义是 intersperse；`join-str` / `.join-str` 返回 String，语义是把 List 元素逐项按原有显示规则转换并插入 String 分隔符。两组调用不能按名称互换。

本批新增 `join-string` 前缀入口与 List `.join-string` 方法。前缀入口包装现有 `join-str`，方法新旧名称均指向 `calcit.core/join-str`。保留 `List<T>, String -> String` 泛型契约；不将输入缩窄为 List<String>，不改变重复值、元素格式化、空 List 的结果或既有失败边界。旧入口暂留兼容。

显式 `core-list-join-string-v1` 只改写 definition `:code` 中完整的 `.join-str separator` 调用；要求具体 List 接收者、新旧方法均 proven、同一实现及相同形参/返回类型，且来源上下文稳定。quoted data、未知 macro、开放接收者及用户方法不自动改写。前缀 `join-str` 的引用不在本规则范围；规则不进已发布 preset。

Calcit definition `:tests` 验证 Number/String 元素、重复值、空 List 及旧新入口等价；Rust CLI 测试只验证预览、应用、幂等、revision 防护、错误分隔符类型诊断及无法自动改写的来源边界。WASM 测试覆盖实际新方法与前缀入口。

本地验证：`calcit calcit/test.cirru test calcit.core/join-string --require-match` 2/2，fix CLI 定向 2/2，`cargo test --locked -q` 全量、`cargo clippy --locked --all-targets -- -D warnings`、`cargo fmt --check`、`yarn check-all`、`bash scripts/test-wasm.sh`、Agent 查询 39/39、文档 73/73 及 Respo 严格类型检查通过。错误分隔符 `Number` 在前缀入口得到类型不匹配诊断。
