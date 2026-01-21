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
