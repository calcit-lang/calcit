# 语义重命名覆盖 definition 附属源码与 schema

## 中文

- 让 `rename-definition-v1` 使用 compiler resolver 追踪 definition-attached `:tests` 与 examples 中的静态引用，在保留测试名称、tags 和 example 顺序的前提下写回。
- 按已加载的 `CalcitTypeAnnotation::TypeRef`、direct trait annotation 与 resolved trait bound 改写 schema，明确区分 nominal reference、泛型 TypeVar 与内建类型，不用 EDN 文本匹配猜测类型含义。
- tests、examples、schema、普通 code、imports 与 definition 声明继续复用同一个 revision-guarded staged transaction；失败时不产生部分修改。
- synthetic `fn` 只提供附属表达式的词法 scope；compiler 仍负责局部遮蔽、import resolution 与 macro origin，quote/quasiquote 和真实 macro 边界继续 fail closed。
- staged validation 额外检查附属源码与 schema 中没有遗留仍可解析到旧 definition 的引用。

## English

- Extend `rename-definition-v1` to trace static references in definition-attached tests and examples through the compiler resolver while preserving test names, tags, and example order.
- Rewrite schemas from loaded `CalcitTypeAnnotation::TypeRef` values, direct trait annotations, and resolved trait bounds so nominal references remain distinct from generic type variables and built-in types instead of relying on EDN text matching.
- Keep tests, examples, schemas, ordinary code, imports, and the declaration in the same revision-guarded staged transaction, with no partial writes on failure.
- Use a synthetic `fn` only to provide lexical scope for attached expressions; the compiler still owns shadowing, import resolution, and macro-origin evidence, while quote/quasiquote and real macro boundaries fail closed.
- Extend staged validation to reject resolved stale references left in attached sources or schemas.
