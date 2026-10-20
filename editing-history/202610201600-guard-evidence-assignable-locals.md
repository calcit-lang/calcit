# 守卫收窄只在局部变量可能被 `set!` 改写时撤销

#2185 让副作用撤销临时的谓词收窄（`list?`、`map?`、`fn?` 等守卫得到的类型），以防被捕获的
`set!` 改写了局部变量。但"副作用"按白名单判断：任何不在只读列表里的调用（`split`、`println`、
`to-string`、普通用户函数）都会清掉调用者所有的收窄。生态模块因此在 alpha.5 能通过、在 main 上
失败，例如 lilac-parser、hn-reader、recollect、respo-markdown 报 `E_CALL_ARGUMENT_UNPROVEN`，
respo 的 `warn-style-literals` 报 `E_DYNAMIC_METHOD_DISPATCH`。

Calcit 的值不可变，局部变量只能被 `set!` 改写。现在预处理每个定义时先收集源码里 `set!` 的目标名，
宏展开后遇到的 `set!` 也加入；副作用和逃逸闭包只撤销这些名字的收窄，loop 也只在有这类收窄时才重查。
没有定义上下文时仍按原来的方式全部撤销。另外把一批不会调用用户代码的纯值 proc（字符串、数学、
集合操作）加入只读列表，让带 `set!` 的 `:js-ffi` 定义也少丢证据。

已知限制：按名字判断，同名的不同局部会被一起视为可改写（偏保守）；反过来，如果某个用户宏在副作用之后
才展开出 `set!`，而源码里没有字面 `set!`，这次撤销会漏掉它。

验证：`scripts/check-strict-default.mjs` 新增三条正向用例（日志副作用、不含写入的未知回调、
可重入 loop），原有的捕获改写反例全部仍被拒绝。
