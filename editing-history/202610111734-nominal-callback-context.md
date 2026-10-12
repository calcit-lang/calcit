# 泛型构造字段保留具体回调输入

## 实际消费者与复现

Respo #195 的泛型 Props/Store 组合在 Map 上下文修复后能执行合法 Op，
但内联 callback 的 d! 42 仍被静态接受，直到 native 执行才失败。最小
Fn<T> dispatch + Props<T>.callback 消费者不含 Map，也复现相同缺口。
这表明不能将 Map 正例通过视为 dispatch 边界已证明。关联 #1553，后续
可将最小例迁入统一改写后校验回归集。

## 两层证据丢失

外层参数已替换泛型实参，但 EXPECTED_STRUCT_TYPE 原来只保存 StructDef，
而 resolve_to_struct_with_ref 返回声明原型时丢弃 applied args。名义构造的
NativeStruct 字段又只读取原型的 field_types，因此回调仍含未绑定 T。
现在同时保存原始名义身份和完整 annotation，消费即时构造上下文，并复用
resolve_struct_field_type_by_index 解析声明 namespace 与替换泛型实参。
只有构造原型与上下文的 nominal identity 一致才提供该上下文；不同类型
仍使用自己的字段声明。没有改写 struct 元数据或 runtime 构造行为。

最初只修正上述字段上下文时，Respo 的具名持有链已拒绝 d! 42，但最小
直接内联 dispatch 仍失败。回归没有删除：Fn 参数的逆变证明遇到期望侧 T
时不能直接从具体 Fn<Op> 获得绑定。对于具体、无自身 generics 的 callable，
且不含未绑定类型变量时，先以既有 compatibility 关系生成候选，只允许
外层 callee 自己的泛型名称，
再用正常严格关系证明替换后的合同。只有严格证明通过才提交候选；没有
改变类型关系的方差、返回值证明或开放值规则，也没有以 compatibility
作为通过依据。泛型 callable 保持现有专门证明路径。

原始 struct-aware map 字段也复用 collection member 上下文入口，使嵌套
Map 回调使用同一规则。存在 spread 后不再按旧 key/value 下标注入上下文。
定义间接编译仍保存/恢复 hint；构造字段进入前消费其即时 hint，避免传给
不相关嵌套构造。新源码注释为英文，没有新增公开 CLI 命令或诊断编号。

## 可执行证据

Calcit definition :tests 在 test-types-inference 的泛型 CallbackHolder 上
断言输入和返回结果，native/JS 专门配对已通过。Snapshot 只用官方编辑
事务的 dry-run/scoped revision 写入。

Rust CLI 回归检验合法具体 Op、错误 Number 在 d! 第一参数拒绝、错误
dispatch/callback 返回拒绝，以及不同名义类型保留自己的 Number 字段
上下文。所有只读检查均验证 Snapshot 字节不变。编译期静态拒绝不能由
普通 runtime 断言表达，因此这部分保留在 CLI 测试。

最初字段修复不足的试跑保留失败结果；最终最小反例的 W_LOCAL_FN_ARG_TYPE_MISMATCH
指向 d! arg 1，并显示具名 Op 与 Number，不通过 runtime 解码来代替它。
Respo 调查脚本增加条件：组合正例接受并执行时，错误 Number 必须在 d!
调用处被拒绝。大型 JSON、临时 Snapshot、诊断、调试输出均在忽略目录。
临时 Rust 跟踪已移除，CLI stdout 合同保持。

同一 65 场景调查已执行：泛型组合正例接受且 native 通过，错误 Number 在
d! 第一参数拒绝；最小直接内联 dispatch 也拒绝同一反例。完整生产源码
strict 仍保留 handle-ssr! 与 render-with! 两项 callable 未证明诊断。

首轮全量 Rust 26 suites、1684 passed、0 failed、1 ignored；补充未绑定
变量保护后的门禁因磁盘 ENOSPC 失败，Rust 的四项、随后十三项失败都在
临时 Snapshot 的原子写回处明确记录 os error 28。共享 core JS 回放亦在
写回 namespace 时失败。失败与观察中断不计作通过，源码断言保持。
清理本工作树旧增量及 debug 缓存、同任务 b0729c62 候选闲置 debug 缓存，
恢复到 6.8 GiB 可用空间；b0729c62 源码/ref/CI 二进制保持，二进制前后
SHA-256 均为 d8cb28395531813445a987bfa0c216727a74af22a0382a52f45193f42b08ecb7。
恢复后首次串行门禁终态为 0：Rust 26 suites、1684 passed、0 failed、
1 ignored；完整 check-all 包括 compile、Agent/API、classification、
core native/JS/WASM 与 IR/WASM/基准均通过。
slot 正例、旧 Tag/cursor-list 兼容与框架完整 strict 仍需独立验收；本修复
不表示 #195 或 milestone 已完成，也不是正式发布工具链 receipt。

## 递归字段的补充反例

随后在忽略副本用 Wrapper<T>.props: Props<T> 再包一层，发现错误
d! 42 仍被接受。根因是通用参数上下文入口只保存/恢复 Fn hint，
构造字段所需的完整 nominal annotation 未递归传入。现在该入口同时
保存/恢复 Struct hint，复用既有名义身份检查；嵌套构造以及集合成员
遵循同一入口，没有追加按类型名分支。

已有 CLI 测试补上嵌套合法 Op 和调用处错误 Number 拒绝，并验证只读
Snapshot 字节不变；新增 CallbackEnvelope<T> definition test 在
native/JS 2/2 零排除回放中通过。原嵌套错误探针现拒绝 d! arg 1。
最终源码串行门禁终态为 0：fmt、all-target clippy、CI CLI 构建、
Rust 26 suites/1684 passed/0 failed/1 ignored 与完整 check-all 全部通过。
Agent 64/64、API 合同 48/48、Dynamic classification 296；core native
617/617、JS 616/616（既有 1 项排除）、WASM 385/385（既有 232 项排除），
IR/WASM 与基准亦通过，没有新增排除项。

最终 Respo 65 场景调查退出 0，行为与未绑定变量保护后的调查相同：
泛型组合正例检查/native 通过，Number 反例调用处拒绝；slot 正例仍失败。
生产 strict 副本重新验证，两项 E_CALL_ARGUMENT_UNPROVEN 保持，safe
fixes 为零。编辑器正式 alpha.29、checker 未发布 0.29.0，仍非发布 receipt。

## 同步 main 与精确合并源码验证

类型修复提交 084fa313 已完成上述全部门禁。读取 main 时最初为
a979b865（#1965 CLI 参数弃用提示），同步时 #1973 已合并为
3b19e411e9d4030d12fa7dab8c5d7d775c42531d。实际无冲突合并提交为
1db797d01eb9d64633e3351651f57ec47381d3b2，同时包含 Snapshot 旧
named-entry 格式恢复，因此重新执行完整门禁，而不把父提交结果当作
合并源码结果。b0729c62 候选 ref 与 CI 二进制不变。

合并后的 fmt、all-target clippy、CI 构建、Rust 26 suites/1686 passed/
0 failed/1 ignored 通过；两项 nominal definition tests native/JS
各 2/2、零排除。Respo 65 场景再次退出 0，行为与 084fa313 相同；
生产 strict 副本仍两项 callable 诊断、safe fixes 零。完整 check-all
终态为 0：Agent 64/64、API 48/48、classification 296、core native
617/617、JS 616/616、WASM 385/385，以及 IR/WASM/基准全部通过。
既有 JS 1 项、WASM 232 项排除保持，没有新增排除项。
新 main 3b19e411 的远端 Test 与 Push on main 均已 completed/success：
https://github.com/calcit-lang/calcit/actions/runs/38132107796
https://github.com/calcit-lang/calcit/actions/runs/38132107246
这些结果按精确 main 提交记录，不沿用旧 main。
