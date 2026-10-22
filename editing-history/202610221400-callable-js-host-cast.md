# js-cast 接受带成员的 callable 宿主

关联 #2153，沿用 `editing-history/202610031245-checked-js-host-cast.md` 的受检宿主转换。真实消费者（Axios、Luxon `DateTime`、Day.js、Chalk、http-proxy 默认导出）都是 `typeof` 为 `function` 的宿主值，旧检查只接受 `typeof` 为 `object`，迫使下游写委托对象或 `unsafe-coerce`。

运行时 `js_cast` 现在也接受函数值，成员检查、错误信息、单次求值与身份保持不变，方法调用仍是 `obj[name](...)`，receiver 就是该函数。Calcit 自身函数编译为普通 JS 函数且无运行时标记，因此 callable 需要至少一个声明成员位于函数自身或非 `Function.prototype` / `Object.prototype` 的原型上，且不是每个函数都有的 `name`、`length`、`prototype`、`arguments`、`caller`；查找用 hasOwnProperty 沿原型链进行，不读取 getter。只满足通用函数成员或无成员的 trait 仍报 `expected a JavaScript host object`，宏与方法闭包同样被拒绝。不新增 SDK 特例、语言类型、静态诊断或 fix 规则；native/WASM 行为不变。

验证：JS FFI fixture 的 definition `:tests` 增加 callable 宿主用例，`scripts/check-js-ffi-source.mjs` 覆盖 class 静态成员与继承静态成员的 receiver、缺成员的既有错误、Calcit 函数/方法闭包拒绝与 Proxy trap cause。限制：跨 realm 函数的 `Function.prototype` 不同，只能依靠成员证据区分。

## English

Related to #2153. Runtime `js_cast` now accepts JavaScript functions (SDK default exports, classes) whose declared members are present, returning the same value so methods keep it as receiver. Because Calcit functions are plain JS functions without a runtime marker, a callable must have at least one declared member found on itself or on a prototype other than `Function.prototype`/`Object.prototype`, excluding the universal `name`/`length`/`prototype`/`arguments`/`caller`; otherwise the existing "expected a JavaScript host object" error remains. No SDK special cases, language types, diagnostics or fix rules were added. Tests live in the existing JS FFI fixture `:tests` and `scripts/check-js-ffi-source.mjs`.
