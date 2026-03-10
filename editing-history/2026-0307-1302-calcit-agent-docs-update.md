# 2026-03-07 CalcitAgent.md 文档更新

## 修改内容

### 1. 新增 `--no-tips` 全局标志文档

在「主要运行命令」小节中，`cr js --check-only` 之后新增：

```
- `cr --no-tips <subcommand> ...` - 隐藏所有编辑/查询命令输出的 "Tips:" 提示行（适合脚本/Agent 使用）
  - 示例：`cr --no-tips demos/compact.cirru query def calcit.core/foldl`
```

**背景**：`aa21b3a` 提交新增了 `--no-tips` 全局开关（`TIPS_SUPPRESSED: AtomicBool`，通过 `suppress_tips()` 设置），文档中缺少记录。

### 2. 新增 HOF 高阶函数回调类型检查说明

在「类型标注与检查 → 3. 支持的类型标签」之后，`约定：动态类型标注…` 注释的下方新增章节：

**高阶函数（HOF）回调类型检查：**  
`foldl`、`sort`、`filter`、`find`、`find-index`、`filter-not`、`mapcat`、`group-by` 等内置 HOF 的回调参数已强制要求 `:fn` 类型，传入非函数值时预处理阶段会触发类型警告。附正/误示例。

**背景**：`aa21b3a` 提交修复了以下两条路径的类型检查：

- Cirru `defn` schema (`calcit-core.cirru`)：`filter-not`/`find`/`find-index`/`foldl`/`foldl'`/`foldl-compare`/`mapcat`/`group-by`/`sort` 的回调参数由 `:dynamic` 改为 `:fn`（部分带泛型注解）
- `ProcTypeSignature` (`proc_name.rs`)：`Foldl`/`Sort`/`FoldlShortcut`/`FoldrShortcut` 的回调参数 `dynamic` → `some_fn()`/`optional_fn()`

## 知识点

- **文档中不需要描述 `ProTypeSignature` 实现细节**，面向 Agent 只需说明行为：哪些 HOF 要求 `:fn` 参数。
- **`--no-tips` 定位**：全局开关，放在 subcommand 前；所有 `Tips::print()` 调用均受其控制。
- CalcitAgent.md 未发现旧 `-s/--stdin` 残留，无需清理。
