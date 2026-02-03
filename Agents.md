## 开发与验证流程

### 核心步骤

- **代码规范**：执行 `cargo fmt` 保持代码格式一致性。
- **质量检查**：执行 `cargo clippy -- -D warnings` 消除潜在风险与性能问题。
- **构建验证**：执行 `yarn compile` 确保前端 TS 与 Rust 核心构建正常。
- **运行测试**：执行 `cargo test` 验证单元测试，`yarn check-all` 验证全量集成测试。

### 功能准则

- **明确边界**：功能改动需考虑边界条件、错误行为及兼容性。
- **一致性**：复用现有模式，保持日志和错误信息风格统一。
- **测试覆盖**：新功能必须补齐正常路径与异常分支的测试用例。

### cr eval 基础与常见踩坑

- **用途定位**：`cr eval` 适合快速验证语义/类型提示与宏展开，不等同于完整项目运行。
- **顶层无需额外括号**：Cirru 语法本身就不需要“最外层括号”，顶层可以直接是表达式。可用 `cr cirru parse-oneliner` 观察解析结果。
  - ✅ `cargo run --bin cr -- demos/compact.cirru eval 'range 3'`
  - ✅ `cargo run --bin cr -- demos/compact.cirru eval 'let ((x 1)) (+ x 2)'`
  - ❌ `cargo run --bin cr -- demos/compact.cirru eval '(range 3)'`（多一层括号会改变调用语义）
- **`let` 绑定语法**：必须用成对列表，形如 `((name value))`。
  - ✅ `let ((x 1)) x`
  - ❌ `let (x 1) x`（会触发“expects pairs in list for let”）
- **告警会使 eval 失败**：有类型告警时，`cr eval` 会以错误退出（这是预期行为，便于阻断不安全用法）。
  - 例：`cargo run --bin cr -- demos/compact.cirru eval '&list:nth 1 0'` 会提示 `:list` vs `:number` 的类型告警。
- **assert-type 仅做检查**：`assert-type` 在预处理阶段生效，不会改变运行值。
  - 例：`cargo run --bin cr -- demos/compact.cirru eval 'let ((x 1)) (assert-type x :list) x'` 依然返回 `1`，并在检查阶段报告类型不匹配。
- **常用排错方式**：遇到报错先看 `.calcit-error.cirru`，它会提供更完整的栈信息。
- **查示例用法**：可用 `cr query examples <namespace/definition>` 查目标定义的示例。
  - 例：`cargo run --bin cr -- demos/compact.cirru query examples calcit.core/let`

### CLI 修改指南与约束

- **优先使用 `target-replace`**：在 `cr tree` 操作中，优先使用 `target-replace` 而非 `replace`。它基于内容定位，且在不唯一时会报错，比手动指定索引更安全。
- **全量取消 stdin (-s) 支持**：由于 Shell 重定向和多行输入的复杂性，所有的修改类子命令（`edit` 和 `tree` 系列）已**移除 `--stdin` / `-s` 选项**。
  - ✅ 使用 `-e 'code'` 或 `-j 'json'` 进行单行输入。
  - ✅ 使用 `-f file.cirru` 进行多行或复杂结构输入（推荐在 `.calcit-snippets/` 下创建临时文件）。
- **路径索引动态性**：在 `tree` 系列操作中（如 `delete`, `insert`），操作会引起同级后续节点索引变化。建议**从后往前**操作，或每次修改后使用 `query search` 重新定位。

## 核心设计约定

### 类型系统 (CalcitTypeAnnotation)

- **去 Option 化**：`CalcitFn`, `ProcTypeSignature`, `CalcitLocal`, `MethodKind` 等结构中，类型标注统一使用 `Arc<CalcitTypeAnnotation>`，不再使用 `Option`。
- **默认通配符**：使用 `CalcitTypeAnnotation::Dynamic` 作为强制性的默认值（替代 `None`），在检查中作为通识通配符。
- **性能优化**：初始化类型列表时，应当共享同一个 `Dynamic` 的 `Arc` 实例（避免 RC 对象重复创建，符合 Clippy 规范）。

## 项目结构概览

- `src/`：Rust 核心实现（`src/calcit/` 数据结构, `src/runner/` 运行时, `src/builtins/` 语法, `src/codegen/` IR/JS 输出）。
- `calcit/`：Cirru 源码与测试用例（`*.cirru`）。
- `lib/` & `js-out/`：JS 共享库与编译输出。
- `docs/` & `demos/`：开发文档与实验性示例。
