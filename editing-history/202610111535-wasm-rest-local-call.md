# WASM 调用带 `&` / `?` 参数的局部函数

关联 #1964、#1961。WASM 直接调用 `fn (a & xs)` 这类局部 lambda 时，内联路径丢掉了 `&` 标记，把 `collect 0 9` 的第二个实参当作 `xs`；通过函数值调用 `+` 等 rest 函数时，`call_indirect` 按实参个数选择类型，也把普通实参当作 rest List 读取。两者都静默返回错误值。

## 决策

内联 lambda 记录参数形状（必填个数、是否有 rest），调用时按 native 的绑定规则处理：缺省的 `?` 参数绑定 nil，`&` 参数收到剩余实参组成的新 List，实参个数不符时仍是编译期 arity 错误：有 `&` 时只有实参少于必填个数才报错；没有 `&` 时，实参少于必填个数或多于全部位置参数（必填加 `?`）都会报错。高阶函数内联回调（`map`、`foldl` 等）、字面量展开调用 `f & $ [] ...` 与立即调用的 `fn`（含 `apply-args`）共用这套绑定；带这类参数的立即调用若函数体使用 `recur`，loop lowering 不会按同样规则重新绑定，因此改为运行到该调用时 trap。静态回调（如 `foldl xs 0 +`）按被调函数的 fixed 参数个数打包剩余实参；缺少 fixed 实参时 trap。

函数表中 `&` 函数的入口改为指向一个 `() -> ()` 的 trap 函数。间接调用使用 `(f64 × N) -> f64` 的规范类型，签名检查必然失败，调用即 trap，不会把实参误读为 rest List；函数值的编码（表 slot 编号）不变，直接调用仍使用原函数。代价是以前碰巧正确的情形也会 trap：经定长 Fn 参数传入 `defn first-of (a & _xs) a` 并以 2 个实参调用，以前得到正确值，现在 trap。没有在间接调用点做运行时打包：那需要为每个表项记录 rest 元数据并在调用点按 fixed 个数分派，属于函数值 ABI 的扩展，留作后续。

## 验证与兼容

新增 core `:tests`：`hint-fn#plain-call-into-rest-local`、`hint-fn#iife-with-rest-param`、`hint-fn#rest-callee-in-hof` 在 native/JS/WASM/WASI 通过；`hint-fn#plain-call-rest-fn-value` 在 WASM 登记为 trap（parity），在 WASI 因间接调用登记为 unsupported。`hint-fn#literal-spread-into-rest-local` 移出排除清单；`reduce#reduces-list-from-initial-value`、`fold#receiver-method`、`assert-type#proven-list-payload` 原先因 `+` 作为静态回调未打包而返回错误值，现在在 WASM/WASI 通过，也移出清单。Rust 测试覆盖函数表 trap 入口的签名失败与字面量展开的打包分支。

用户可见变化：WASM 上这些调用现在返回与 native 相同的值，或在运行到该调用时 trap，不再返回错误值。
