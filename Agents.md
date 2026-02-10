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
- **顶层无需额外括号**：Cirru 语法本身就不需要"最外层括号"，顶层可以直接是表达式。可用 `cr cirru parse -e` 观察解析结果。
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
- **常用的编辑操作**：
  - `edit mv <source> <target>`：移动或重命名定义。
  - `tree cp <target> --from <path> -p <path> [--at <pos>]`：在 AST 节点间复制内容，支持 `before`, `after` (默认), `prepend-child`, `append-child`, `replace`。

## 性能与资源验证

### 技术基准

- **启动耗时验证**：使用 `time ./target/release/cr calcit/test.cirru -1`。核心库加载应控制在 **~10ms**。若数值大幅增加，需检查 `build.rs` 的序列化或 `include_bytes!` 是否失效。
- **构建体积监控**：使用 `ls -lh target/release/cr` 观察产物。目标应控制在 **5MB** 以内。引入新依赖前务必检查其 Transitive Dependencies，优先选择同步轻量库（如 `ureq`）。
- **IO 纯净度检测**：确保 stdout 仅保留程序逻辑输出。版本信息、预热日志、告警均应通过 `eprintln!` 输出至 stderr。验证：`./cr -v > /dev/null` 不应有任何输出。

### 内存性能检查

- **高频分配规约**：在 Preprocessing 阶段，严禁在循环内调用 `Arc::new(CalcitTypeAnnotation::Dynamic)`。应始终 clone 单例 `crate::calcit::DYNAMIC_TYPE`。
- **验证手段**：对于大规模 Cirru 项目的预处理，可通过 `repeat 10 { time ./target/release/cr ... }` 观察耗时抖动。若抖动剧烈，通常预示着堆内存申请频率过高或冷热数据加载策略存在问题。

## 项目结构概览

- `src/`：Rust 核心实现（`src/calcit/` 数据结构, `src/runner/` 运行时, `src/builtins/` 语法, `src/codegen/` IR/JS 输出）。
- `calcit/`：Cirru 源码与测试用例（`*.cirru`）。
- `lib/` & `js-out/`：JS 共享库与编译输出。
- `docs/` & `demos/`：开发文档与实验性示例。
