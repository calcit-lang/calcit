# Ref 构造首选名 ref / defref（#1457）

`type-of` 返回 `:ref`、谓词为 `ref?`、文档与 watcher 契约写 `Ref<T>`，构造却叫 `atom` / `defatom`。维护者在 #1457 选定：0.29 起首选 `ref` / `defref`，提供受控 fix 规则，旧名保留到下一个非 patch 版本。

实现：不新增平行实现。`CalcitProc::Ref` 以 `to_string = "ref"` 为显示名，`serialize = "atom"` 让旧拼写读入同一个 proc；`CalcitSyntax::Defref` 同理接受 `defatom`。因此 native 求值、类型推导（`Ref<T>`、空初始化固定 payload）、macro capability、JS codegen（`ref` / `defatom` runtime helper）与 WASM 全局 lowering 只有一条路径。JS runtime 导出 `ref`，`atom` 作为同一函数保留给已有 JS 调用方。源码级工具（effects graph、type coverage、snapshot 定义头、edit 重命名）同时识别两种拼写。

兼容边界：
- `atom` / `defatom` 不加 `:deprecated`。Respo、memof、js-ffi、respo-ui 有 legacy `analyze quality` baseline，且每个定义的 `deprecatedCalls` 为 0；加标记会让这些项目的 CI 直接失败。
- 读取器把 `ref` 解析为内建 proc（与 `atom` 一样），名为 `ref` 的局部绑定或参数会报错。core 的 `add-watch!` / `remove-watch!` 参数已改为 `target`。抽查 Respo、respo-ui、alerts、reel、respo-markdown、calcit.std、js-ffi、lilac、memof、cumulo-workflow、TopixIM/diary 的默认分支，没有局部 `ref` 绑定，也没有名为 `ref` / `defref` 的定义。
- 运行时错误消息与 WASM unsupported 诊断中的 proc 名改为 `ref`；内部 Ref 路径前缀 `atom-N` 与 Cirru EDN 的 `atom` 表示不变。

fix 规则 `core-ref-constructor-v1`：`atom` 是读取器层面的同一 proc，代码中被求值的 `atom` 直接改写；`defatom` 只在调用头、无同名局部/ns 定义/import、不在 quasiquote 模板、外层无未知 macro 时自动改写，否则 `requires-review`。quote、cirru-quote 与注释跳过。支持 `--include-attached`。新 preset `core-api-0.29-v1` = `core-api-0.28-v1` 的 13 条 + 本规则，已发布 preset 含义不变。

验证：core `ref#creates-typed-local-ref` 在 native/JS 重放（WASM 与 `atom` 一样排除）；`calcit/test.cirru` 的 `*defref-demo` 在 native/JS；`calcit/test-wasm.cirru` 的 `defref` / `defatom` 全局由 `scripts/test-wasm.mjs` 在 WASM 执行；`tests/fix_cli.rs` 覆盖正负例、附带测试、幂等与 preset 组成。
