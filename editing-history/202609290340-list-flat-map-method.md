# List 展平映射的公开方法

关联 #1455。

List `.bind` 与前缀 `mapcat` 当前都调用 `calcit.core/mapcat`：回调接收每个元素并返回 List，结果按原顺序展平一层。Fn `.bind` 则指向另一实现，不能全局按名字迁移。新增更可预测的 List `.flat-map`，保持相同的泛型 `T -> List<U>` 回调与 `List<U>` 返回契约，不引入惰性 iterator 或 ownership。

显式 `core-list-flat-map-v1` 只扫描所选 definition 的 `:code` 中完整的 `.bind callback` 方法调用。自动迁移要求接收者是具体 List、旧新方法均为 proven 且指向同一 core 实现、形参与返回类型相同，并且来源结构可追溯。quoted data 不扫描；未知 macro、开放 receiver 与用户方法不自动改写。规则不进入旧升级 preset；前缀 `mapcat` 和旧 `.bind` 暂保留，等待实际消费者迁移与版本窗口再讨论移除。

Calcit definition `:tests` 验证输出顺序、异类型映射、空 List、回调次数及旧方法兼容；Rust CLI 测试只验证预览、revision 防护、应用、幂等和不能自动迁移的来源边界。Agent 查询核对旧新方法的类型及实现证据。

本地验证：`calcit calcit/test.cirru test calcit.core/mapcat --require-match` 2/2；`cargo test --locked -q` 全量通过；`cargo test --locked --test fix_cli list_flat_map_fix` 2/2；`cargo fmt --check` 与 `cargo clippy --locked --all-targets -- -D warnings` 通过；`bash scripts/test-wasm.sh` 覆盖实际 `.flat-map` 方法并通过；`yarn install --immutable && yarn procs-link && yarn compile && yarn check-all` 通过；Agent 查询 39/39、文档 73/73；Respo 严格类型检查通过。PR CI 待补充。
