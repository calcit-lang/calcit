# query usages 按 import 规则解析 namespace alias

#2236 报告：调用方以 `app.schema :as schema` 导入后写 `schema/decode-story`，`query usages app.schema/decode-story` 仍输出 0。原实现只检查 namespace 声明中是否出现目标 namespace 名称；出现时仅搜索裸名 `decode-story`，否则仅搜索完全限定名。因此 `:as` 导入的 alias 调用全部漏报；同一 namespace 同时 `:refer` 并写完全限定名时，完全限定调用也会漏报；只 `:as` 导入时，同名的局部参数反而被误报。

现在每个 namespace 用编译器同一份 `extract_import_rule` 解析 `:require` 规则（不打印 warning，忽略格式错误的规则，后面的绑定覆盖前面的），得到目标定义在该 namespace 中的全部写法：完全限定名始终计入；每个指向目标 namespace 的 `:as` alias 计入 `alias/def`；`:refer` 了目标定义或位于目标 namespace 时计入裸名。code、`:tests` 与 schema 三处共用同一组写法，坐标按路径排序合并。

边界：查询仍然基于符号拼写，不追踪 macro 生成的引用；局部遮蔽造成的误报由 #2164 单独处理，本次不改变。没有新增命令、flag、输出字段或诊断编号；`query context` 的 usages 复用同一收集函数，一并修正。

验证：`tests/fixtures/query-usages-alias.cirru` 覆盖 alias（code 与 tests）、`:refer`、同 namespace 并存的完全限定调用、无 import 的完全限定调用、目标 namespace 内的裸名，以及只 `:as` 导入时不匹配裸名参数；`tests/query_usages_cli.rs` 断言 6 处引用。修复前同一 fixture 报告 4 处，其中包含 1 处误报。
