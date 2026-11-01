# Trait 字段类型的同命名空间短名

#2502：`deftrait Host (:style 'Style)` 中的 `Style` 是同一 namespace 的 external-object trait。读取 `host :style` 时结果被推断为类型变量 `Style`（confidence generic），而不是名义 trait，于是 `js-set style :width |100px` 不走 external-object 字段赋值，生成 `$clt.js_set(style, _t_.width, …)`，在 JS 对象上写入 `":width"` 属性。strict 检查不报错。把字段写成 `'app.main/Style` 后结果正确。

原因：trait 成员类型由 `parse_type_annotation_form`（非 strict 模式）解析。这个模式下，不带 `/` 的引用名一律当作类型变量。`deftrait` 和 `&trait::new` 都不声明泛型 binder，所以这里的短名永远不会是真正的泛型，结果只是一个未绑定、没有字段证据的类型变量。源码侧解析（`parse_deftrait_source`）和运行时 `&trait::new` 走的是同一条路径。

决定：解析 trait 成员类型之前，用 `qualify_trait_member_type_form` 把引用短名补全为限定名，然后交给原解析器。名字的查找方式与 trait `requires` 相同：先找同一 namespace 的定义，再找 `:refer` 导入。builtin 类型、`*slot`、找不到定义的名字保持原样；`Fn`/`Macro` schema（compact 或 canonical 形式）也不改写，因为它们有自己的 `:generics`，由 schema 解析器处理。两条路径共用这个函数，所以静态检查和运行时 trait 值得到同一个名义身份。没有新增类型关系或诊断编号。

同时补了一道门禁：receiver 只推断为类型变量、key 是 Tag 时，`js-get`/`js-set`/`aget`/`aset` 与裸 `JsObject` 同样报告 `E_UNTYPED_JS_OBJECT_ACCESS`。这覆盖了字段类型名拼错、找不到定义的情况，避免再次静默写入 `:width`。字符串 key 和动态 key 保持原来的 raw JavaScript 语义。

兼容边界：已经限定的写法不受影响。短名原本解析成未绑定类型变量，没有任何检查依赖它，所以改为名义引用只会让推断更精确。生成代码只在原来错误的位置发生变化。

验证：`calcit/test-js.cirru` 的 `test-nested-external-field` 对比了未限定的 `TestStyleHost` 与限定的 `TestQualifiedStyleHost`，由 `yarn try-js` 在 Node 中实际运行并检查宿主对象的 JSON。修复前这个用例失败，实际结果为 `{"width":"",":width":"100px"}`。`calcit/type-fail/unbound-trait-field-tag-key-strict.cirru` 及对应的 Rust 测试 `strict_type_fail_unbound_trait_field_tag_key_reports_stable_error_code` 覆盖找不到定义时的拒绝。
