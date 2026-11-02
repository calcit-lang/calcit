# 嵌套 union 解码保持成员来源

在生态迁移复核中，用两个 namespace 的同名 Scalar 复现：内层 String/Number/Tag List 被外层 Bool/Symbol 声明遮蔽，导致合法 List 被拒绝、非法 List 被接受。对应上游 PR #2531 记录。

在既有 UnionMember 保留展平前的 declaring_namespace，与集合检查 PR #2533 使用完全相同的字段与实现；build_union 对非名义成员使用此来源构建闭合数据形状，缺失时才回退外层。只修正已有关系，不新增 API、证明规则、诊断、ABI版本或脚本。

原 test-union fixture 新增两个声明 namespace、四个 union 定义与一项共享测试，覆盖原四个 checked decoder 入口的正反例。native/JS 各7/7、零排除；原27个定义和完整元数据的官方namespace视图保持。完整冻结源码门禁和最新CI仍需完成。

Preserve each flattened union member's declaration namespace when building checked data shapes. The shadowing control reproduces both false rejection and false acceptance. Reuse the identical member provenance field from #2533; no new API, proof rule, diagnostic, ABI version or script. The original fixture gains one shared regression across all four checked-decoder entry points, passing 7/7 on each native/JS backend without exclusions while preserving all27 original definitions and metadata. Full frozen-source gates and latest CI remain required.
