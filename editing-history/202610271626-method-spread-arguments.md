# 静态方法 lowering 保留实参展开

Manuscript 的 0.31.0 迁移复现了 `m .dissoc key & keys` 在 native 只删除固定 key：
后置方法已选出 `&map:dissoc`，但构造直接调用时没有保留 `CallSpread`。
相同遗漏也影响 List 的 `.concat`，不能在 Map 删除器里特判修补。

统一在构造已选择的调用时保留展开标记，覆盖后置、前置、trait origin 和无法内联的
方法路径。类型证据仍用于选择目标；不改回开放 native call，不扩大 Dynamic，也不新增
迁移规则或诊断编号。后置语法复用现有的非 List 展开检查。
该检查同时要求每个 `&` 都有操作数，兼容模式也不能静默忽略末尾或连续标记。

WASM 的 Map 删除器原来只读取展开列表第一项，混合参数还会把 `&` 当成表达式。
改为按原顺序求值固定参数与展开 List，逐个删除所有 key；空 List 不贡献 key，
总共没有 key 时仍失败。Map 接收者和每个参数只求值一次。
不透明展开值复用已有 heap magic/tag 判断，在指针转换与 count/元素读取前确认 List；
静态已证明的 List 保留直接路径。原边界脚本覆盖有效/空 List、Map、Set 和标量的真实执行。

WASM 的字面量 List 展开还需复用普通调用的类型 specialization。普通同型 Map merge
能够执行，若直接调用未 specialization 的 rest 函数体却会生成无效模块；字面量展开后
沿用普通调用路径，未知长度的运行时展开仍走原有的受检协议。
此复用还要求已有证据证明函数目标，未知函数表目标仍由原 ABI 门禁拒绝；
声明为固定 Fn 本身不能证明实际实现不是 rest 函数，原宿主测试继续守住该边界。
字面量 normalization 前也保留已有 nil/trait 展开拒绝门禁，确保失败仍由原有诊断说明；
不能让普通 specialization 入口绕过展开调用自身尚未支持的证明边界。

回归放在 definition `:tests`：直接/后置方法、精确的 Map/String/List 参数合同、
多次与空展开、List concat 和 Map merge。已有 Ref 求值顺序测试保留原断言，扩展方法参数顺序和
展开失败后的短路。
复用原统一回放器和 spread 边界脚本，不新建运行器，不新增或放宽后端排除。

验收包括修复前后的同源回放、原生/JS/WASM/WASI 0.3 command、完整 Cargo/Clippy、
主仓库 check-all 和 agent interface。Ref/try 与其他开放值的既有 WASM 限制仍如实保留，
不把其他后端通过解释为这些能力已经实现；具体验证结果记录在 PR。
