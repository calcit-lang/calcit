# 同质 Map 内联回调的上下文证明

## 真实阻塞与最小复现

Respo #195 的泛型 Props/Store 组合正例在字段内联 callback 处被拒绝：
字段期望 Map<Tag, Fn<…>>，实际只保留 Map<Tag, Fn>。此前源码候选已让
基本泛型 Ref 和 Store 持有链通过；组合正例仍失败，不能据此选择 dispatch 路线。

以 Fn<Map<Tag, Fn<Number> Unit>> 为参数的最小消费者复现：普通检查接受，
完整 strict workflow 报 E_CALL_ARGUMENT_UNPROVEN。回调的参数上下文在
同质 Map 字面量边界丢失，完整源码审计不能独立重建证明。

## 修改与兼容边界

沿用 List/Set 的 collection member 上下文入口，为完整、无 spread 的 &{}
字面量逐个传入 Map 的 key/value 上下文。保留值的独立推导与返回类型检查，
没有把期望返回类型写成实际证明；错误 String 返回仍在 strict workflow 拒绝。
不完整字面量与 spread 表达式沿用原处理路径。没有修改 runtime dispatch、
业务 Op、slot 绑定、语言语法或错误编号。关联 #1553，后续可迁入统一改写后校验集。

新定义测试放在 calcit/test-types-inference.cirru，直接调用类型化 Map 中的
callback 并检查 Number 结果。Rust CLI 回归覆盖完整 strict 正例、错误返回
拒绝和 Snapshot 字节不变；编译时拒绝无法由普通 runtime 断言表达。
Snapshot 通过官方 edit transaction 的 dry-run 和 scoped revision 守卫写入。

## 共享运行器的必要修正

native 定义测试通过后，既有共享运行器在执行前因单 namespace 的紧凑
Snapshot 布局失败。运行器现查询完整 namespace 数据，在内存中加入回放
定义，再用官方 edit ns 批量写回，不依赖序列化文本的 namespace/defs 缩进。
每次先 dry-run，再用 scoped revision apply。临时 namespace 从 Snapshot package
派生，维持源码所有权；原 core package 的名称与 permission 行为保持。
namespace payload 由现有 Cirru parser/writer 编解码，schema 保持 CIRRU 数据。
两个早期试跑分别在 namespace ownership
与 schema payload 转换处失败，均未执行测试，不能计作通过。
没有新增回放脚本，临时事务、Snapshot、JSON 诊断与日志均留在忽略或临时目录。

## 已完成的局部验证

- 最小消费者完整 strict 从一项未证明诊断变为 diagnostics=[]。
- 新定义测试 native 与 JS 各 1/1 通过，无排除。
- strict_workflow_preserves_inline_map_callback_contracts 通过。
- Respo 既有 holder 模板捕获的真实泛型组合正例通过 check-only 和 native 执行。
  该回归使用正式 alpha.29 编辑器与未发布 core checker，不是正式依赖 receipt。

同 slot callback 关系、异质 raw props 与旧 Tag/cursor-list 调用兼容尚需验证；
这次 Map 修复本身不能宣称完成 #195，也不能宣称 milestone 已验收。

## 完整场景的边界核对

同一组 65 场景运行退出 0。泛型 Ref、基本 Store/Props 和组合 Props 正例
均接受且 native 通过，基本组错误 Op/State 仍拒绝。但是组合内联 callback
中的 d! 42 仍被检查接受，native 才失败；调查脚本绿色不能当作安全验收。
构造字段还没有获得外层实参的具体泛型绑定。仅让外层
参数使用 substitute_type_vars 的试验没有修复此路径，已撤回。
进一步用不含 Map 的泛型 Props.callback 最小消费者隔离：合法具名 Op
通过检查与 native，内联回调 d! 42 也被检查接受。该缺口独立于本次 Map
字面量处理。query type-at 的部分证据仍显示 d! 为 Fn<T> Unit，绑定不完整；
该查询本身不是通过证明。官方事务复现保存在忽略目录，后续需保留构造字段的
具体泛型上下文。

Respo 6a21ba5 完整源码 strict 仍有两项 E_CALL_ARGUMENT_UNPROVEN：
handle-ssr! 的具名 Op 回调与开放 Fn<Dynamic> 入口不匹配、render-with!
向 render! 传递裸 Fn；safe fixes 为零。Map 局部修复没有掩盖这两项。

首轮全量 Rust 门禁通过：26 个 suite，1683 passed、0 failed、1 ignored。
共享运行器改写时误删了 WASM 读取仍需的 readFile 导入，首次 check-all
在 native/wasm short-circuit 配对中报告 readFile is not defined。已恢复
该导入，最终门禁需要完整重跑；首次失败不计作通过。

逐定义事务版本虽通过完整 native 617 项，但含生成的时间达到 425.6 秒，
暴露每项 edit 启动一个子 CLI 的开销；该轮剩余后端已主动停止，不计全绿。
改为 namespace 批量写回后，native 同一 617 项耗时降为 6.9 秒；这两次为
单次端到端观察，不是 compiler 性能基准。edit ns 自带原子写回，当前不能
作为 edit transaction 子命令；运行器据此使用逐 namespace 的 dry-run/apply，
新增 namespace 和 mode 配置仍由原 guarded transaction 完成。

## 最终门禁

最终 CALCIT_LINT_CORE=1 CALCIT_BIN=target/ci/calcit yarn check-all 完整退出 0：
Agent interface 64/64、API baseline、core Dynamic classification 296 positions、
原生 core unit 588 项，以及共享 native 617/617、JS 616/616、WASM 385/385
通过。共享 JS 原有 1 项与 WASM 原有 232 项排除保持，没有新增排除。
外部 namespace 的 trait 配对、类型推导测试、native/JS/IR/WASM 和现有
literal-path / typed-method 回归均通过。新增 Map 定义测试还通过 native/JS
各 1/1 的专门回放。格式、差异检查、CI 构建与 all-targets clippy 通过。

前述退出失败或主动停止的试跑均保留日志用于定位，不计入最终通过结果。
先前待授权的 b0729c62 core 与 b09ecbb memo ref 保持原提交，本次只整理
fix/map-callback-context-195 的本地候选；没有外部推送、评论、重开或发布。
