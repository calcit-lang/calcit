# 裸 `Enum` / `Struct` / `Trait` / `Fn` 的提示

类型位置里的裸 `Enum` 解析为 `AnonymousEnum`，`Struct` / `Trait` 解析为 `Custom(:struct)` / `Custom(:trait)`，`Fn` 解析为 `DynFn`。它们不完全等同于 Dynamic：作为期望类型时接受任意 enum / struct / 函数，但不带具体定义，读出的载荷、字段和调用结果都是 Dynamic，流入具体类型时还需要 `UnresolvedNominalIdentity` 边界。生态普查（216 个去重仓库的默认分支）中 134 个仓库有这类写法，共 846 处：Enum 673、Fn 135、Struct 38；`Trait` 全部是 `deftrait` 自身的定义种类标记。典型写法是 dispatch 处理函数 `Fn Enum (-> Unit)`、`reel.typed/State Enum Store`、回调字段的裸 `Fn`，另有 48 处 `defstruct` / `defenum` 的 `:schema` 误写成 `Enum` / `Struct`。

先按 #2497 的做法报告非阻塞提示，后续非 patch 版本再升级为错误。与 #2497 不同，这里没有自动改写：具体类型只有作者知道，编译器不选择业务类型。

检查放在 Snapshot 层（`snapshot::collect_broad_type_hits`），只扫描项目自身的 namespace，加载后由 `cr.rs` 推入 advisory 通道，`--check-only` 与 `--all-defs` 的 Notes 都能看到。代码位置用 Cirru 遍历，类型槽与 `edit format` 的迁移一致（`assert-type`、`unsafe-coerce`、`hint-fn`、解码目标、函数返回标注、`defstruct` 字段首项、`defenum` 载荷、`deftrait` 成员，包括包在一个 `{}` 里的成员写法），`quote` 内的数据不扫描；带参数的头部（`Fn Number (-> String)`、`:: 'Fn $ {} …`）不算裸。`:schema` 已在加载时解析成 annotation，直接遍历 annotation 组件，因此 `'Enum`、`:enum`、`quote Enum` 等拼写都被覆盖。定义级 `:schema` 的根按代码头部判断：`deftrait` 的 `Trait` 豁免，`defstruct` / `defenum` 的根若是值族，则提示改为 `StructDef` / `EnumDef`。

新增诊断码 `W_BROAD_TYPE`：`W_LEGACY_TYPE_SYNTAX` 表示“有等价精简拼写、可由 `edit format` 改写”，这里是语义上的过宽类型，没有等价改写，混用会让迁移命令的含义不清。

`calcit.core` 的底层 proc（如 `&enum:assoc`、`&struct:impl-traits`）需要对任意 enum / struct 工作，保留宽泛签名；它们不属于用户项目的 namespace；扫描本身也排除 `calcit.core`，单独检查 core Snapshot 时同样不提示。

验证：`cargo test --lib broad_types_are_reported_with_their_place` 覆盖各类型槽、坐标、定义种类与不提示的情况；放在 Rust 是因为它测的是 Snapshot 扫描，`:tests` 无法观察检查 Notes。`calcit/test.cirru --check-only` 报告 6 条提示并通过。
