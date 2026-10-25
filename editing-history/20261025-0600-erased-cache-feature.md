# 类型擦除缓存的恢复边界 `:erased-cache`

## 问题与依据

#2358：recollect 的 `memo-twig-by0/1/2` 声明返回 `'R`，内部调用存取 Dynamic 的变长 `memo-twig-by`。返回证明正确地拒绝了 Dynamic 到 `'R`；改用 `assert-type` 时 strict 报 `E_ASSERT_TYPE_UNPROVEN`，而 `unsafe-coerce` 只允许在 `:js-ffi` 范围内，且在那里只有 host handle 目标算证据。库作者没有合法写法表达“这里从类型擦除的缓存恢复值”。

## 修复边界

新增函数 schema feature `:erased-cache`。在其词法范围内：

1. `unsafe-coerce` 不再报 `E_UNSCOPED_UNSAFE_COERCE`；
2. strict 证明把 `unsafe-coerce` 的目标类型当作结果（普通检查本来如此），因此泛型返回得到证明。

它不授予 JS FFI 能力：`current_function_has_js_ffi_feature` 不变，`js-cast`、host 访问、Dynamic 调用 lint 照旧。`:js-ffi` 里的非 host 目标仍不算证据。strict workflow 仍把这些转换列为 `explicit-unsafe` 待审项。`edit schema --add-feature` 接受 `erased-cache`。`E_UNSCOPED_UNSAFE_COERCE` 的提示同时指出这一写法。

不新增原语或诊断，不放宽 `assert-type`。

## 验证

新 fixture `calcit/type-fail/unsafe-coerce-erased-cache-strict.cirru`（与 recollect 同形：泛型 `'R` 从回调返回值得到，缓存层是 Dynamic）在 strict workflow 下 passed；去掉 feature 报 `E_UNSCOPED_UNSAFE_COERCE`，换成 `:js-ffi` 报 `E_ERASED_GENERIC_RELATION`。预处理单测覆盖允许、拒绝和“不授予 js-ffi”。

另见：用户泛型函数的类型参数只出现在回调返回值时，调用点不会从回调实参推出它（主线已如此，与本改动无关），所以 fixture 的调用方不依赖该结果类型。
