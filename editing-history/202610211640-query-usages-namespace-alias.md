# query usages 按 import 规则解析 namespace alias

#2236 报告：调用方以 `app.schema :as schema` 导入后写 `schema/decode-story`，`query usages app.schema/decode-story` 仍输出 0。原实现只检查 namespace 声明中是否出现目标 namespace 名称；出现时仅搜索裸名 `decode-story`，否则仅搜索完全限定名。因此 `:as` 导入的 alias 调用全部漏报；同一 namespace 同时 `:refer` 并写完全限定名时，完全限定调用也会漏报；只 `:as` 导入时，同名的局部参数反而被误报。

现在每个 namespace 用编译器同一份 `extract_import_rule` 解析 `:require` 规则（不打印 warning，忽略格式错误的规则，后面的绑定覆盖前面的），得到目标定义在该 namespace 中的全部写法：

- 完全限定名默认计入；调用方把目标 namespace 名本身绑定为其他 namespace 的 alias 时不计入。
- 每个指向目标 namespace 的 `:as` alias 计入 `alias/def`。
- 裸名在目标 namespace 内计入；在其他 namespace 中，当该名称由 `:refer` 从目标 namespace 导入时计入，若后续规则以 `:refer`、`:as` 或 `:default` 重新绑定则不计入。
- 目标为 `calcit.core` 时，裸名在任意 namespace 计入，除非 import 规则或该 namespace 自身的同名定义重新绑定它。

schema 中的类型名是带 `'` 的符号（`'Story`、`'schema/Story`、`'app.schema/Story`），比较前去掉前导 `'`，此前 schema 引用实际上从不匹配。code、`:tests` 与 schema 三处共用同一组写法，坐标按路径排序合并。结果为 0 时追加一行说明 macro 生成的引用和局部遮蔽不在检测范围，零命中不能证明未使用。

边界：查询仍然基于符号拼写。局部遮蔽造成的误报由 #2164 处理；`:refer` 名称被 core 名或调用方自身同名定义遮蔽的情况也未建模。没有新增命令、flag、输出字段或诊断编号；`query context` 的 usages 复用同一收集函数，一并修正。

验证：`tests/fixtures/query-usages-alias.cirru` 覆盖 alias（code 与 tests）、`:refer`、同 namespace 并存的完全限定调用、无 import 的完全限定调用、目标 namespace 内的裸名、只 `:as` 导入时不匹配裸名参数，以及 schema 中 alias/refer/完全限定三种带引号写法；`tests/query_usages_cli.rs` 精确断言计数与零命中提示。`target_reference_symbols` 的单元测试用手写 ns form 覆盖后续规则重新绑定（`:refer`、`:as`、`:default`）、同一 namespace 两个 alias、`-v2` 前缀边界、目标 namespace 名被重新绑定以及 `calcit.core` 裸名。修复前 fixture 对 `decode-story` 报告 4 处（含 1 处误报），修复后 6 处。
