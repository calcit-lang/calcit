# 2026-03-05 类型推断增强记录

## 本次目标

- 在 upstream 重置后，重新加强类型推断能力，并保证 debug/release/全量检查都稳定通过。

## 关键改动

### 1) 预处理阶段 Proc 返回类型传播增强

在 `src/runner/preprocess.rs` 的 `infer_type_from_expr` 中补齐多类内建过程返回类型推断：

- List 保型：`sort`、`&list:concat`、`&list:assoc`、`&list:assoc-before`、`&list:assoc-after`、`&list:dissoc`。
- 固定返回：`range -> list<number>`，`split/split-lines -> list<string>`，`&map:to-list -> list`。
- Map 保型：`&map:assoc`、`&map:dissoc`、`&merge`、`&merge-non-nil`、`&map:diff-new`。
- Set 保型：`&include`、`&exclude`、`&difference`、`&union`、`&set:intersection`。

### 2) defn 参数类型提示传播修复（避免栈溢出）

在 `src/builtins/syntax.rs` 中增强 `defn` 的 `arg_types` 回退策略：

- 第一层：沿用原逻辑，从 body 中 `assert-type` 模式提取。
- 第二层：当全是 Dynamic 时，从参数列表 Local 的 `type_info` 读取。
- 第三层：当仍全是 Dynamic 时，从**预处理后 body 顶层 Local**提取（`assert-type` 被预处理成 typed local 的场景）。
- 为避免核心加载路径栈压力，`calcit.core` 命名空间跳过该额外回退扫描。

### 3) calcit-core 类型提示细化

在 `src/cirru/calcit-core.cirru` 为以下列表保型函数补充泛型提示：

- `distinct`
- `drop`
- `repeat`
- `reverse`
- `take`
- `take-last`

统一使用 `hint-fn (generics 'T) $ return-type (:: :list 'T)`，并将输入列表参数改为 `assert-type ... (:: :list 'T)`。

### 4) 测试用例一致性调整

`calcit/test-types.cirru` 中 `test-arg-type-hints` 示例从故意错误参数改为合法参数，避免新检查路径把 warning 升级为阻断导致全量检查失败。

## 过程中的关键排障结论

- 多次尝试在 `preprocess_defn` 内直接重建/替换参数 AST 或引入额外全局缓存时，容易触发主流程栈溢出。
- 最终采用“最小干预 + 避开 core 热路径回退扫描”的方式稳定通过。

## 验证结果

- `cargo test` 通过。
- `yarn check-all` 通过（`EXIT:0`）。
- `cargo build --release` + `./target/release/cr calcit/test.cirru -1` 通过。
- `./target/debug/cr calcit/test.cirru -1` 通过。
