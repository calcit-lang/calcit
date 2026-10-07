# 显式开放值进入具体参数的证明

#1767 的复现：`List<Dynamic>` 参数的元素传给 `Fn(Number)->Number` 的局部函数，严格预处理通过，错误直到 `&+` 运行时才出现；`Data :number (&list:nth items 0)` 也被接受。原有严格检查只在名义类型（`E_DYNAMIC_NOMINAL_ARGUMENT`）和泛型关系（`E_ERASED_GENERIC_RELATION`）上要求 Dynamic 的证明，标量与容器等具体参数直接接受。

改为在共同的调用证明 `find_unproven_generic_argument` 中补上具体参数分支：同一 TypeProof 给出 `NeedsBoundary(Dynamic)` 时报告既有的 `E_CALL_ARGUMENT_UNPROVEN`，位置定位到实参（lowering 后失去 head 位置的嵌套调用从其 operand 推出 list 坐标）。Enum 构造把 variant payload 当作构造器参数、enum 泛型当作调用泛型，复用同一函数，因此 `Option :some` 等泛型传播保持不变。

`Dynamic` 目前同时表示用户选择的开放值和推导无法表达的证据（Map 条目 pair、匿名 enum payload、上下文回调参数、`apply` 结果、空容器）。全面收紧会在 core 文档与测试中误报这些位置，因此本规则只认显式开放值：由 `hint-fn` 或 schema 声明的参数，以及经 `&let` 从这些参数计算出的开放值。`explicit_open` 以词法作用域记录标记，每个绑定都会写入（含遮蔽），离开作用域恢复。泛型关系与名义检查的既有范围不变；assertion audit 不增加 enum payload 检查。

实参定位从 operand 推导坐标时只接受与调用方同一定义的位置；`fn` 等 macro 展开引入的节点带有 macro 自身的位置，不能借用。

`tests/fixtures/fix-command.cirru` 的 `fixable` 实际接收 Enum 值（其 test 传入 `%some 1`），原 schema 却声明 `Dynamic`。新规则下 `removed-data-api-v1` 迁移后的 `enum-definition value` 在普通严格检查即被拒绝，staged fix 无法写入；改为声明 `'Enum` 给出真实证明，相应的 fix CLI 断言不再期待迁移后的 `E_CALL_ARGUMENT_UNPROVEN`。

验证：`calcit.core/hint-fn` 新增三个 `:tests`（谓词收窄、`data-view`/Data 构造、泛型传播），并随 `check-known-assertion.mjs` 在 native 与真实 JS 回放；同一脚本新增四个负例（直接元素、Dynamic 参数、`let` 别名、`Data :number` payload），在 native、check-only、JS、WASM、WASI 预处理阶段拒绝并检查实参坐标。core unit tests 与文档 check-md 无需修改既有用例。Respo 当前 main 的严格检查新增两处同类诊断（`comp-task` 的 `states` 游标进入 `Op :states`，`add-prop` 的 Dynamic `prop-value` 进入 `style->string`），均为真实的未证明边界。
