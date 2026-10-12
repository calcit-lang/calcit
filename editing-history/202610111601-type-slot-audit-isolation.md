# 类型槽声明与源码审计的状态隔离

## 真实阻塞

Respo #194/#195 的完整 strict workflow 在仅有一处 dispatch-op 声明时报告
type slot already declared。用既有 deep-recursion fixture 和官方 edit def
加入一处 deftype-slot 可复现；两个入口显式返回 Unit 后，仍只剩该重复声明。
单独的一次 assertion 审计不报错，完整 workflow 的多个源码审计会报错。

## 根因与取舍

with_assertion_proof 为外层审计保存编译缓存，强制重编译源码以验证独立证据。
deftype-slot 在预处理时注册声明，而该注册表此前没有与编译缓存一起隔离。
后续审计会把上一轮审计或普通编译的声明误认作第二处声明。

外层审计现以空声明注册表开始，并用 Drop 恢复原注册表；嵌套审计共享外层
注册表，成功、错误与 unwind 都恢复原状态。entry 的 type-slots 绑定和词法
with-type-slot override 保持有效。没有把 register_type_slot 改为幂等：同一轮
中两个不同定义声明同名槽仍被拒绝。此次仅修正审计状态，语言和 runtime 的
重复声明规则保持，Respo 的业务 Op 及 legacy Tag dispatch 尚需独立解决。

## 测试位置与验证

注册表、编译缓存与 panic 恢复属于宿主内部 invariant，使用 Rust 测试；CLI
测试通过官方编辑接口构造 fixture，检验整个 workflow 接受一处声明、拒绝
两个真实声明，并保持 snapshot 字节不变。不新增回放脚本或公开命令。

- 三项 assertion_audit 内部回归通过，覆盖绑定、override、嵌套与异常恢复。
- strict_workflow_replays_type_slot_declarations_without_hiding_duplicates 通过。
- cargo fmt 与 yarn compile 通过。
- cargo clippy --profile ci --all-targets -- -D warnings 通过，CI CLI 构建通过。
- cargo test --profile ci 完整退出 0：26 个 suite，1682 passed、0 failed、1 ignored。
  HTTP/WS fixture 使用已获准的本机临时端口权限；未改变断言或排除网络测试。
- CALCIT_LINT_CORE=1 CALCIT_BIN=target/ci/calcit yarn check-all 完整退出 0，包含
  Agent CLI、API baseline、core classification、definition tests、native/JS/IR/WASM
  与现有 literal-path/typed-method 回归。最终 cargo fmt --check 和差异检查通过。
- Respo 6a21ba5 的完整源码副本回归：相同 revision
  md5:ae383f77c9d3652074e3c39ab2233a12 从三项诊断降为两项，重复槽诊断消失；
  safe fixes 为零。剩余 handle-ssr! 的 Op 回调与开放入口关系、render-with! 的
  裸 Fn 调用证据仍报 E_CALL_ARGUMENT_UNPROVEN，完整 strict 仍失败。

Respo 副本仅声明稳定 0.29.0 以运行未发布 core，正式 alpha.29 依赖保持；
这不是正式工具链 receipt。
此分支基于待审 b0729c62，
待审分支保持原提交；所有复现 JSON、日志和工具链副本留在忽略目录。
