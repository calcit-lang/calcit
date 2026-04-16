# 202604161507 - WASM 数据结构编译 & rfc 目录重命名

## rfc 目录整理

- `drafts/` 重命名为 `rfc/`，所有文件加上 `MM-DD-` 创建日期前缀（从 git 历史提取）
- README.md 更新为完整索引，补全之前遗漏的 5 个文件条目
- Cargo.toml 中 `drafts/` 引用更新为 `rfc/`

## WASM 数据结构编译

### 新增能力

- **Memory Section**: 1 页 64KB 线性内存，Global section 维护 `heap_ptr` 作为 bump allocator
- **Tag 编译**: `collect_all_tags()` 在编译期为所有 tag 字面量分配整数 ID，运行时直接用 f64 表示
- **Record 编译**: `emit_record_new()` 在堆上分配 `[struct_tag_id, field0, field1, ...]`，`emit_record_nth()` 通过偏移直接读取
- **Tuple 编译**: `emit_tuple_new()` 在堆上分配 `[tag_id, payload0, payload1, ...]`，`emit_tuple_nth()` 直接读取
- **静态结构解析**: `try_parse_defrecord_form()` 从源码 AST 静态提取 `defrecord` 的字段定义，不依赖运行时宏展开

### 关键设计决策

- 全 f64 ABI：所有值（包括指针）用 f64 传递，需要 `i32.trunc_f64_u` / `f64.convert_i32_u` 转换
- record 字段按字母序排列（与 Calcit runtime 一致）
- bump allocator 不回收，适合短生命周期 WASM 模块

### 测试覆盖

- 22 项 WASM 检查全部通过：17 个数值运算 + 2 个 tag 比较 + 2 个 record 求和 + 1 个 tuple 求和
