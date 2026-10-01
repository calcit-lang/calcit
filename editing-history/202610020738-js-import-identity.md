# JS 跨 namespace 引用保留来源身份

## 问题与复现

直接限定的跨 namespace 定义被 JS emitter 生成裸 named import。同一个名字同时存在于本地、
两个依赖 namespace 或 lexical scope 时，可能造成 ESM 重复声明，或读取错误的局部变量。
这属于 backend 语义丢失，不要求业务改名，也不通过补写生成文件或 npm 路径绕过。

现有 typed-source-alias fixture 新增两个依赖与 consumer 的 Number `state-alias`，
用 definition `:tests` 同时读取本地、直接限定、namespace alias 与同名 let 局部值。
全部六个 native 附带测试通过；同一测试树写入临时入口后 native 执行通过，旧 JS 实际
Node 加载报告 `Identifier 'state_alias' has already been declared`，形成无循环复现。

## 实现与验证目标

跨 namespace 的 NsReferDef 引用使用已有 namespace binding 与属性读取，和 NsAs 合并
到同一个 namespace import 去重逻辑。同 namespace 仍保留局部引用，不产生 self-import。
模块开始生成前收集实际 codegen 中的 source bindings；内部 namespace binding 避开这些名字，
并对已经占用的 namespace binding 继续消歧。测试覆盖主动与内部 binding 同名的合法 `$` 局部变量，
以及 escape 后同名的两个不同 namespace，不仅把原来的裸 def 冲突转移到 namespace 名字上。
同一 namespace 的显式限定引用还暴露了独立的局部遮蔽：native 读取模块值 10，JS 错读 let 值 7。
保留该断言，针对存在局部同名绑定的模块定义使用模块作用域 getter；getter 不求值 initializer，
保持已有初始化顺序、引用身份与无 self-import 行为，普通未遮蔽定义仍直接引用。
不新增公开 API、命令或运行器，扩展已有 typed-source-alias 的 native/实际 Node 回放。

后续验证包括新共享测试、已有 same-namespace 与 nominal alias 回归、JS FFI 导入、
完整 fmt/Clippy/Rust/Node 集成以及安装后 Respo 回归。WASM 未增加能力，不声称新用例在
WASM 执行；完整门禁尚未执行完之前不宣称修复完成。
