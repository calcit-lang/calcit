# Reproducible Component core WASM / 可复现的 Component core WASM

## 中文

- WASM lowering 不再直接使用随机种子 `HashMap` 的 namespace 与 definition 遍历顺序分配函数索引、table slot、import 和 atom global。
- 编码顺序固定为入口 namespace 优先、其余 namespace 按名称排序；每个 namespace 内的 definitions 同样按名称排序。
- Component boundary 与 WASI target 的预检复用相同排序，使生成物和首个诊断都保持稳定。
- 新增跨两个独立 Calcit 进程的逐字节回归测试，直接使用发布版 WASI HTTP 业务示例生成 `program.wasm`。

## English

- WASM lowering no longer assigns function indices, table slots, imports, or atom globals from randomized `HashMap` namespace and definition iteration.
- Encoding keeps the entry namespace first, sorts remaining namespaces by name, and sorts definitions within every namespace.
- Component-boundary and WASI-target validation reuse the same ordering so artifacts and first diagnostics remain stable.
- Add a byte-for-byte regression across two independent Calcit processes using the published WASI HTTP business example to generate `program.wasm`.
