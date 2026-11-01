# Callback 参数在名义门禁中逆变

#2499：strict 模式下的 Dynamic/名义参数门禁（`E_DYNAMIC_NOMINAL_ARGUMENT`）比较 `Fn` 类型时，参数位置和返回位置用同一方向递归，于是 `Fn Dynamic (-> Unit)` 传入 `Fn FsPath (-> Unit)` 参数槽被当作“开放数据进入名义合同”拒绝。实际数据流相反：参数槽的调用方只会传入 `FsPath`，这些值流入实际函数的 `Dynamic` 参数，属于安全的放宽。

修复只在 `dynamic_erases_nominal_contract` 的 `Fn` 分支交换参数（含 rest 参数）的比较方向，返回值保持协变。参数按位置配对：固定参数缺失时回退到 rest 类型，所以只有 rest 参数的 callback 与固定参数槽之间也会检查（原先固定参数个数不同就跳过整个分支，`Fn & FsPath (-> Unit)` 可以漏进 `Fn Dynamic (-> Unit)` 槽）。结果：

- `Fn Dynamic (-> Unit)` 进入 `Fn FsPath (-> Unit)`：通过。
- `Fn FsPath (-> Unit)` 进入 `Fn Dynamic (-> Unit)`：参数槽会传入 Dynamic 值，流入名义参数，报告 `E_DYNAMIC_NOMINAL_ARGUMENT`（此前这个方向在本门禁里漏判，只靠定义级 proof 的 `E_CALL_ARGUMENT_UNPROVEN` 兜底）。
- rest 参数同理：`Fn & Dynamic (-> Unit)` 进入 `Fn & FsPath (-> Unit)` 或 `Fn FsPath (-> Unit)` 通过；`Fn & FsPath (-> Unit)` 进入 `Fn & Dynamic (-> Unit)` 或 `Fn Dynamic (-> Unit)` 报告 `E_DYNAMIC_NOMINAL_ARGUMENT`。
- `Fn FsPath (-> Dynamic)` 进入 `Fn FsPath (-> FsPath)`：返回值仍按协变检查，继续报告。
- arity 不符时，只比较两边都有的位置；多出的参数继续由普通参数类型检查报告 `W_FN_ARG_TYPE_MISMATCH`。

没有新增诊断编号、fix 规则或 proof/audit 机制；`contains_dynamic_type` 等其余分类不变。参数槽是非名义类型（如 `Number`）时本门禁不涉及，这一方向（#2116）仍由定义级 directional proof 负责。

验证：`calcit/type-fail/callback-contravariance-strict.cirru` 的 `default`/`reverse`/`result`/`arity`/`reverse-rest`/`rest-for-fixed` 六个 entry 由 `cargo test --bin calcit strict_callback_parameters_are_contravariant_at_the_nominal_boundary` 检查；`strict_types_reject_dynamic_values_crossing_into_nominal_arguments` 单元测试改为断言两个方向与 Dynamic 返回值。拒绝路径属于预处理诊断，`:tests` 只在检查通过后运行，所以放在 type-fail fixture。
