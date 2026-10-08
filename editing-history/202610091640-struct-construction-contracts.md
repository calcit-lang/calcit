# Struct 构造入口复用数据字段合同

## 依据

#1868 与前置 #1869 的审查指出：字段更新开始深检，但动态构造仍会收下 `List<Number>` 中的 String。使用正式 alpha.20 CLI/npm 配对和公开 `%{}` 动态构造器、`&struct:from-map` 回放同 AST，两个负例在 native/JS 均错误接受。raw `&%{}` source 本来会被 strict 拒绝，不能把这种编译拒绝当成运行时证据；正式用例使用公开构造器。

## 实现

native 完整构造复用既有字段检查，保留原泛型 binding/where-bound 校验；from-map 转交完整构造，不再独立维护浅检查或给遗漏字段填 nil。JS 完整构造检查字段类型、规范化重复键，并由 from-map 复用同一实现。from-map 保持公开签名中的 Map 输入，不保留 JS 特有的 Struct 输入和多余字段忽略行为；使用数组而非参数展开，避免大 Map 引入调用参数数量限制。

验证同时发现 native 的零字段 Struct 定义/构造被旧 arity 检查拒绝，而公开 runtime arity 与 JS 均允许；修复为只要求 prototype/name。没有新增公开入口、诊断编号、固定改写或业务类型强转。

## 验证与边界

既有 def-value-schema fixture 新增 27 组 Calcit `:tests`，经 CLI query、transaction dry-run 与 revision 守卫写入，加入原 check-struct-field-js。覆盖动态 prototype、Tag/String/Symbol 字段名、标量/嵌套容器、同名异源 nominal、alias、具体泛型、Option payload、显式 Dynamic、缺字段、多余字段、规范化重复、不可变输入和零字段构造。保留原 40 组更新及静态错误拒绝/WASM 回归。

这一步未将旧的任意动态构造/写入登记为完整静态证明。未实例化泛型、Fn/host 合同、局部 JS prototype metadata 及 Recollect/Diary 原 strict 门禁仍由 #1868 跟踪；不能扩大 With 例外来绕过它们。只有运行时合同真正可验证，才允许后续 strict 认可相关动态边界。
