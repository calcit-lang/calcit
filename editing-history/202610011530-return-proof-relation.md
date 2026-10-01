# 返回值检查与执行体对齐

关联 #1538、#1553。

macro 生成的函数会追加一参数 `hint-fn` schema 元数据。runtime 已剔除这些提示，返回值检查却读取原始 body 的最后一个节点，因而可能漏掉真正的返回表达式；同时提前跳过 DynFn，允许函数值被声明为 Number 等非函数类型。

返回值检查复用 runtime 的元数据判定，选取最后一个可执行表达式，并使用已有 TypeProof。确定矛盾沿用 W_FN_RETURN_TYPE_MISMATCH；需要边界证明的情形继续走既有兼容迁移路径，不改变消费者全面收紧的发布顺序。一参数元数据与两参数表达式提示仍按既有语义区分。

发现 WASM codegen 没有阻断预处理 warnings；直接加门禁同时暴露现有 async Component 的逻辑返回类型与 AsyncInvocation 类型不对齐。这项能力需单独按 #1533 / #1538 验证，当前修复不改 WASM 入口，也不把 unsupported 当成返回值证明。

合法 callable、显式 Dynamic 存储及明确 Unit 返回写入 hint-fn 的附带测试。七类错误返回值复用既有 assertion 运行器，在 native、check-only、JS 核对拒绝与诊断；既有 assertion 的 native/JS/WASM 负例与 scalar WASM 正例保留。待 #1553 落地后纳入其统一改写回归集。
