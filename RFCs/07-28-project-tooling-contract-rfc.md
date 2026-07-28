# RFC: 在既有 cr 子命令上完善单项目开发体验

状态：Draft
日期：2026-07-28
关联：`07-28-git-module-store-rfc.md`、`07-26-safe-structured-editing-rfc.md`

## 1. 目标

在不引入 workspace、不改变 EDN snapshot 事实来源、也不重塑为 Cargo 风格命令面的前提下，完善单项目日常体验。开发者与 Agent 应通过既有 `cr` 子命令知道如何检查、构建、测试、格式化与校验文档；仓库内部的 Rust/Yarn 脚本不应成为普通 Calcit 项目的前置知识。

## 2. 建议命令面

不新增 `new`、`fmt`、`check`、`test`、`build` 这类顶级命令。能力应归入已有、语义最相近的子命令；先统一参数、JSON result 与文档，再视真实缺口增加小的子命令。

```bash
cr <snapshot> edit format                 # 既有 snapshot 规范化写入
cr js --check-only                         # JS target 的静态验证，不执行入口
cr analyze check-types --format json       # 类型覆盖与 schema 检查
cr analyze weak-types --format json        # 定位未解决/有意动态边界
cr analyze check-examples --ns <ns>        # 运行 definition examples
cr --entry test                            # 运行项目已有 test entry
cr docs check-md <markdown>                # 校验文档 Cirru 代码块
cr docs graph check                        # 校验文档关系图
cr js                                  # 使用已有 target 子命令一次性构建 JS
cr ir                                  # 使用已有 target 子命令一次性构建 IR
cr wasm                                # 使用已有 target 子命令一次性构建 WASM
```

若需要新项目起点，应维护最小示例、模板仓库或文档步骤，而不是把脚手架扩展成新的核心 CLI 契约。测试、文档和源码 metadata 保持 definition/section 层级，以便 query、tree edit 和 Git review 复用。

## 3. 构建与验证原则

- 静态检查归入 `js --check-only` 与 `analyze`；这些检查不运行项目 init function；
- 测试继续由显式 entry（通常为 `--entry test`）和 `analyze check-examples` 承担，而不引入另一套 test discovery 规则；
- target 构建继续使用 `js`、`ir`、`wasm`；一次执行与显式 watch 的既有语义不变；
- 所有机器结果使用统一 JSON envelope，正常 stdout 不混入提示；
- target、entry、module resolution、resolved commit 与版本选择 warning 必须出现在结果 metadata；
- 可在既有 entry/analyze 参数上补充筛选、definition/tree identity 与稳定 failure code，不能另造平行工作流；
- 文档代码块由结构解析与静态/执行检查验证，避免文档只靠人工复制；
- coverage、benchmark、profile 可后续加入，但不阻塞上述闭环。

## 4. 非目标

- 不仿造 Cargo 风格的顶级项目命令、workspace、features resolver 或 crate 多版本模型；
- 不把源文件按行切分为另一套事实来源；
- 不要求立刻以 LSP 取代 CLI/workflow。

## 5. 验收

一个项目从 `caps`、`cr <snapshot> edit format`、`cr js --check-only`、`cr analyze check-types`、`cr --entry test` 到所需的 `cr js|ir|wasm` 有一条明确且可脚本化的路径；任一失败可通过 definition/tree 语义位置而非脆弱行号定位；现有 JS/IR/WASM 一次执行与显式 watch 语义保持兼容。
