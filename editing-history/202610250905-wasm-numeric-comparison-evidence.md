# WASM 数值排序保留真实操作数证据

关联 #2342、#1553。名义 Enum 的 `.contains?` 可以被解析到实际 trait 实现，因此只在动态方法 emitter 检查 key，不能覆盖这个内联路径。scalar ABI 又把 nil、Bool、Tag 和 Number 擦除为 f64；不能从 0/1 或指针范围重新证明 Number。

## 决策

- 数值排序原语 `&<`、`&>` 复用现有类型证明关系。已证明 Number 才生成浮点比较；已知类型不匹配时，先按原顺序执行全部实参，再 trap；真正开放的操作数明确拒绝 codegen。保留原有错误传播，不在表层增加严格检查规则。
- `identical?` 仍使用原通用比较 emitter，不套用 Number 合同。Map 的 nil/Bool key 不借用 Enum 的下标规则。
- 使用已有 scalar 调用点特化传递具体类型，标记 Number 判断及需要操作数证据的比较。不能从方法名或被调用者的期望参数倒推实参类型；失败诊断保留实际特化函数名，便于追踪外层调用。
- 调用点已证明 Number 时，`number?` 保留该证据并仍执行实参一次，不再从 f64 的位值猜测对象类别。合法数字可能恰好等于活跃堆对象地址；现有 WASM 宿主测试分配真实对象并把同值作为 Number 下标，分别验证直接方法与泛型 helper 返回越界结果，而不是错误 trap。
- `number?` 调用的操作数在函数内已有 Number 证明时，不为无关泛型参数触发 scalar 特化；扫描仍检查操作数自身的嵌套敏感表达式，独立传递的谓词值仍保守处理。复审用实际 Int8 refinement 与无关泛型参数的递归函数复现误拒绝，普通 Number 用例作为正控。此处只消除不必要的特化，不增加递归特化能力或放宽开放类型。
- 补齐既有类型流失点：rest 参数在后端保留为 List；集合原语用当前实参证明泛型返回；比较循环把初值和成员共同证明的类型传给生成的临时参数。循环累加器后续来自前一成员，不能只采用初值类型。临时参数的名称和类型元数据都恢复词法遮蔽。
- 特化传递实参的真实类型，不要求与当前操作无关的所有参数先闭合。使用 nil 判断、数值排序或 trait 分派时，各 emitter 仍必须独立取得证据，否则明确拒绝；不会把开放值当成已证明 Number。这个统一规则也覆盖 Nil 的空 payload 与 Never 的不产生值路径，不再逐项添加 phantom generic 例外，求值仍执行原错误路径。
- 回放未被入口调用的定义时，发现 Snapshot 补全会生成新 nominal callable，却在旧 clone 中丢失其导出。冻结 codegen 视图前合入补全期间实际编译的定义，同时保留 Snapshot-only fallback；不靠调整 namespace 顺序或应用调用方式修补。现有 Enum-cycle fixture 增加未被入口引用的定义和 native/JS 失败语义回放。

## 验证边界

共享正常语义放在 `contains?` 的 definition `:tests`：Number 下标的负零、分数、边界、无穷和 NaN，泛型 helper，以及 Map key / identity 正控。WASM 不支持 Calcit `try`，非法 key 和求值顺序复用已有 `test-wasm` fixture 与宿主断言，不新增运行器、CI job 或后端排除。

初步扩大验证曾暴露原比较循环把临时参数写成 Dynamic；该失败保留在调查记录，修复类型传递后重放原测试，不修改原断言。完整 Cargo、core 跨后端、原 WASM、Agent interface 和全量 check-all 的最终结果记录在 PR，不把定向通过当成全量通过。

公开 `contains?` 的结果 Bool 证据仍由 #2339 单独验收。显式内部 `&trait-call` 对晚生成 callable 的收集也另有边界；本次不把这些路径宣称为已完成，不删除兼容入口、添加 unsafe/Dynamic 或发布版本。
