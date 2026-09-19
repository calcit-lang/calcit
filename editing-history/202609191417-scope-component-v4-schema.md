# 收紧 Component v4 stream schema / Scope the Component v4 stream schema

## 中文

- 将 `readable-byte-stream` 限定在异步 Component export 的直接参数中。
- 让 import、同步 export 与所有 result 继续引用 v3 类型 schema，从发布的 JSON Schema 层面 fail closed。
- 增加 schema 分支测试；编译器测试继续覆盖 import、同步 export、嵌套参数与 result 的负例。

## English

- Restrict `readable-byte-stream` to direct parameters of asynchronous Component exports.
- Keep imports, synchronous exports, and every result on the v3 type schema so the published JSON Schema fails closed.
- Add schema branch coverage while the compiler tests continue to cover import, synchronous export, nested-parameter, and result negative fixtures.
