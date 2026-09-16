# WASM Cirru EDN 顶层标量 Map 格式化

- `format-cirru-edn` 首批支持顶层闭合标量 `Map<K,V>`，key/value 限定为 nil、Bool、String、tag 与整数 numeric refinement，避免把开放类型或不精确 Number 带入运行时猜测。
- WASM 复用 `__rt_map_linearize` 取得独立扁平缓冲区，再按闭合 key 类型执行稳定插入排序：String 使用现有字节序比较，tag 利用按名称分配的稳定 id，其余标量使用数值顺序。
- 只输出能够与 native compact formatter 保持完整字节一致的顶层标量 Map；嵌套 Map 与容器 value 暂时以稳定诊断失败，避免忽略 Cirru formatter 的 `$` 折叠与多行布局规则。
- Calcit definition `:tests` 表达 tag/String Map 的用户语义，WASI smoke 继续逐字节比较 native 与 Wasmtime stdout；Rust 测试只覆盖允许/拒绝的低层 schema 边界。
