# 受控迁移效果方法名

#1457 已在 core 提供 FsPath 和 native FFI 生命周期的带 `!` 方法，但旧方法仍被真实消费者调用。新增一个显式选择的 `core-effect-method-v1`，复用现有 method alias 规划器，不为每个旧名增加单独检测入口。规则同时要求 nominal 接收者、core trait 来源、旧新方法完整签名与相同实现目标；只替换源码 method leaf，因此保持接收者和参数的求值次数及顺序。

Cirru reader 将 `task.cancel-with` 等紧凑 leaf 展开为方法调用，故规则同时识别紧凑与显式形式。无法稳定回到源码的未知宏只给人工审阅，quoted 数据跳过；开放接收者和同名用户方法没有充分证据时不自动写入。规则仅处理 definition `:code`，不悄然改写 attached tests/examples，也不加入已发布 preset。

验证需覆盖预览、revision 前置条件、应用后复查为空、Calcit definition 测试、Agent `query type` 的首选/兼容角色、全量 native/JS/WASM 回归。native FFI 的 exactly-once 和释放由已有宿主边界测试继续验证；名称迁移不新增 JS/WASM capability。
