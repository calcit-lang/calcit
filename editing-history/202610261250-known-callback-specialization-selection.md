# 已知回调独立触发调用点特化

## 原因与选择

#2179 的普通 Set `map` 用例在 native、JS、WASM 通过，WASI 仍报 `E_WASI_COMMAND_INDIRECT`。预处理正确选择了 `&set:map`，它的回调也有具体 Fn 参数合同；但其函数体没有需要特化的标量判断或 trait 调用，实参也不是匿名闭包，因此原选择条件直接使用通用编译函数体，没有进入已有的真实参数目标保存路径。

复用现有静态调用特化：固定参数、无 `recur`、不在当前展开栈中的定义，只要实际 Fn 参数具有已知函数定义目标，也选择原特化路径。类型签名只定位回调参数，不能证明 callee 身份或宿主能力。参数仍由现有代码按原顺序求值一次，使用已有 fresh local、别名及闭包捕获元数据。

没有新增公开 API、ABI、诊断、checker 或永久改写，也没有扩大 Dynamic 或添加 unsafe。未知函数目标不触发这条路径，命名递归不重复展开；可达宿主能力校验保持原样。带 `recur` 的参数可能在回边改绑，不凭初始目标启用此特化。

## 验证与范围

- 在原函数值 fixture 增加有完整合同的 `invoke-number-key` 及七项 definition `:tests`，覆盖具名回调、不可变别名、同签名遮蔽、独立调用、Set map、空 Set 和值参数求值一次。沿用既有回放 tag 和运行器；正常语义不改成 native call。
- 七项在 native、JS、WASM、真实 Wasmtime WASI 各 7/7，零排除；旧 main 的固定 CLI 对同源源码 native 7/7、WASI 七项均编译拒绝，保留真实基线对照。
- 完整 core native 761/761、JS 759/759、WASM 493/493、WASI 492/492。只删除原 `maps-over-sets` 的一条 WASI 排除，官方 EDN 解析与逐路径深比较确认其他数据不变。
- 扩展原 callee identity Rust 测试，不新增 Rust 测试数量：回调身份本身触发特化；同形但未知目标保留普通调用；可达不支持宿主仍拒绝；活动展开栈不重入。原未知/宿主/recur 控制保持。
- `cargo fmt --check`、`git diff --check`、all-target Clippy、低层边界测试和原完整 WASM 脚本通过。完整 Cargo 与完整 check-all 的最终结果在 PR 中记录，不由窄回放代替。

另两类原间接调用（函数组合返回闭包、捕获 MapEntry 的排序）没有因本次成功而视为完成；#2179 保持开放。固定 CLI 是源码候选，不冒充正式发布配对验收。
