# 命名类型泛型绑定统一 RFC

日期：2026-06-01

状态：Draft

## 背景

当前类型系统已经支持：

- 函数级泛型变量 `TypeVar`
- `:where` trait 约束
- `Struct` / `Enum` / `TypeRef` 三类命名类型表示
- 在 `matches_with_bindings` 中边匹配边收集泛型绑定

但是这条链路还不够统一。

最明显的缺口是：当两个命名类型本质上表示同一个定义，但只有一侧携带了已应用的泛型参数时，绑定行为并不一致。

- `Struct(Applied)` 对 `Struct(Bare)`：当前已经会把已应用参数绑定回声明的泛型变量。
- `TypeRef(Applied)` 对 `TypeRef(Bare)`：当前基本仍是宽松通过，不一定留下可用绑定。
- `Struct(Applied)` 对 `TypeRef(Bare)`：本轮已经补齐绑定。
- `Enum(Applied)` 对 `TypeRef(Bare)`：仍然缺失，因为 `CalcitEnum` 还不携带泛型变量名。

这会导致一个问题：调用点表面上“类型匹配成功”，但后续依赖绑定结果的能力并没有拿到足够信息，例如：

- 泛型返回类型特化不够稳定
- `:where` 约束检查可能看不到完整实参
- 不同命名类型组合的行为不一致，用户难以建立心智模型

## 目标

把“命名类型匹配时如何产生泛型绑定”统一成一条简单规则，尽量贴近 Rust：

- 先确认两侧是否是同一个命名类型
- 如果两侧都带泛型实参，则逐项统一
- 如果只有一侧带泛型实参，则把实参绑定回类型定义声明的泛型变量
- 匹配成功不应只是 `true`，还应尽可能留下后续阶段可复用的 bindings

这里的“贴近 Rust”不是复制 Rust 的完整 trait solver，而是借鉴它的一致性原则：

- 同一个类型构造器，在不同语法表面下应走同一套统一规则
- 泛型参数一旦可从已知实参恢复，就应该恢复，而不是跳过

## 非目标

本 RFC 不覆盖：

- 完整 trait 求解器
- 高阶类型或 higher-kinded types
- 复杂 `where` 传递闭包推导
- monomorphization 策略变更
- 运行时表示改造

## 当前问题拆解

### 问题 1：单边已应用泛型时，命名类型匹配过于宽松

当前若一侧是 bare type，另一侧是 applied type，经常直接返回 `true`，等价于“你们名字一样，那先算匹配”。

这在弱检查阶段很方便，但它牺牲了后续信息。

例如：

```text
actual:   Struct(Pair, [number, string])
expected: TypeRef("Pair", [])
```

如果这里只返回 `true` 而不写入：

- `A -> number`
- `B -> string`

那么后续如果某个返回值、字段访问或 `:where` 约束依赖 `A` / `B`，就只能退化到更弱的判断。

### 问题 2：不同命名类型组合的统一规则不一致

当前已经存在这样的不对称：

- `Struct` 对 `Struct` 有“单边 applied 时绑定泛型”的逻辑
- `Struct` 对 `TypeRef` 过去没有同等级逻辑
- `Enum` 对 `TypeRef` 目前也没有

这意味着用户只是换了一层命名表示，行为就变了。

从工程上看，这种不一致比“暂时保守”更难维护，因为：

- bug 不稳定复现
- 某些路径上 `:where` 警告会出现，另一些路径不会
- 推理链条难以复用

## 本轮已落地的最小修复

本轮已经在 `Struct <-> TypeRef` 上落地了最小统一：

- 两边都 bare：只检查命名是否一致
- 两边都 applied：逐项匹配参数
- 一边 bare、一边 applied：把 applied 参数绑定回 `CalcitStruct.generics`

也就是从“宽松通过但不留痕”改成“通过且留下绑定”。

## 详细案例

### 案例 A：`Struct(Applied)` 对 `TypeRef(Bare)`

输入：

```text
actual   = Struct(Pair, [number, string])
expected = TypeRef("Pair", [])
```

期望行为：

- 匹配成功
- bindings 写入 `A -> number`, `B -> string`

原因：

- `Pair` 已经由结构定义声明了泛型变量顺序
- 已应用实参信息就在 `Struct` 上，跳过绑定没有收益

收益：

- 后续返回类型、字段类型或 `:where` 检查可以继续消费这组绑定

### 案例 B：`TypeRef(Applied)` 对 `Struct(Bare)`

输入：

```text
actual   = TypeRef("Pair", [number, string])
expected = Struct(Pair, [])
```

期望行为：

- 匹配成功
- bindings 写入 `A -> number`, `B -> string`

原因：

- 这和案例 A 在类型论上没有本质区别
- 只是 applied 参数出现在另一侧

### 案例 C：`TypeRef(Applied)` 对 `TypeRef(Bare)`

输入：

```text
actual   = TypeRef("app/Pair", [number, string])
expected = TypeRef("app/Pair", [])
```

建议行为：

- 如果 `app/Pair` 可 resolve 到带泛型定义的 schema，则应绑定 `A -> number`, `B -> string`
- 如果无法 resolve，则保留当前宽松行为或显式降级策略

难点：

- `TypeRef` 自身只保存名字和参数，不直接携带定义处的泛型变量名
- 因此需要借助 schema lookup 才能知道“第 0 个参数其实是 `A`”

这是下一步值得推进的点。

### 案例 D：`Enum(Applied)` 对 `TypeRef(Bare)`

输入：

```text
actual   = Enum(Result, [number, string])
expected = TypeRef("Result", [])
```

建议行为：

- 匹配成功
- bindings 写入 `T -> number`, `E -> string`

当前阻碍：

- `CalcitEnum` 当前不保存泛型变量名列表
- 因此即使拿到了 `[number, string]`，也不知道应当绑定回哪个变量名

这说明 enum 侧不是“缺一行匹配代码”，而是缺底层元数据。

## 为什么这更像 Rust

Rust 在处理泛型时有一个非常强的直觉：

- 只要类型构造器确定，参数信息就应该尽可能参与统一
- 统一得到的结果要继续喂给后续约束求解与返回类型推导

例如在 Rust 里：

```rust
fn id_pair<A, B>(x: Pair<A, B>) -> Pair<A, B> { x }
```

当调用点给出 `Pair<i64, String>` 时，编译器不会因为另一侧写的是 `Pair<A, B>` 就只判断“名字一样”。它会把：

- `A = i64`
- `B = String`

完整带入后续链路。

Calcit 当前的问题不是“没有泛型”，而是“某些路径下统一得不彻底”。

## 优点

### 1. 行为更一致

同一个命名类型，不再因为表面写成 `Struct` 还是 `TypeRef` 就触发不同绑定规则。

### 2. `:where` 约束更可靠

很多 `:where` 检查都依赖前一步先拿到绑定结果。绑定越完整，约束检查越不容易漏报。

### 3. 返回类型特化更稳定

如果泛型返回值引用了 `A` / `B`，统一链路越完整，返回类型越不需要退回 `dynamic`。

### 4. 便于后续扩展

后面无论是继续改 `TypeRef <-> TypeRef`，还是补 enum 泛型元数据，都可以沿同一原则推进，而不是为每个组合单独发明例外。

## 缺点与风险

### 1. 会暴露更多已有问题

一旦绑定更完整，后续 `:where` 或返回类型检查就可能出现更多 warning。这不是新 bug，而是旧问题被看见了。

### 2. enum 侧需要结构升级

要让 `Enum <-> TypeRef` 也做到同等级统一，需要 `CalcitEnum` 记录泛型变量名。这个改动比当前 struct 侧修复更深。

### 3. `TypeRef <-> TypeRef` 可能引入 schema lookup 成本

如果每次都 resolve schema，会增加匹配成本，也要注意避免循环解析与缓存失效。

### 4. 兼容性上会更“严格”

过去某些路径只是宽松 `true`，不会留下更多信息。统一后可能触发更精确的 downstream 检查，用户会感觉类型系统突然变严格了。

## 建议的增量落地顺序

### 阶段 1：补齐 `Struct <-> TypeRef`

这一步已经完成。

落点：

- `matches_with_bindings`
- 抽出复用 helper，避免同类分支各写一遍绑定逻辑

### 阶段 2：补齐 `TypeRef(Applied) <-> TypeRef(Bare)`

建议策略：

- 仅在命名可 resolve 到 schema 时启用绑定
- 无法 resolve 时维持当前保守行为

这样不会把整个 `TypeRef` 系统一次性改成强解析。

### 阶段 3：给 `CalcitEnum` 增加泛型名字元数据

完成后再把：

- `Enum <-> Enum`
- `Enum <-> TypeRef`
- `Tuple(sum type) <-> TypeRef`

统一到同一套绑定原则下。

### 阶段 4：观察 warning 面

在 bindings 更完整之后，重新评估：

- `W_GENERIC_WHERE_BOUND_MISMATCH` 是否明显增加
- 哪些 core schema 仍旧过宽
- 是否需要把某些 warning 升级为 hard error

## 最小测试建议

至少保留以下方向：

1. `Struct(Applied)` 对 `TypeRef(Bare)` 会留下 bindings
2. `TypeRef(Applied)` 对 `Struct(Bare)` 会留下 bindings
3. `TypeRef(Applied)` 对 `TypeRef(Bare)` 在可 resolve 时会留下 bindings
4. `Enum(Applied)` 对 `TypeRef(Bare)` 在补齐 enum 元数据后会留下 bindings
5. 新 bindings 会被 `:where` 检查真实消费，而不是只存在于匹配函数内部

## 开放问题

1. `TypeRef <-> TypeRef` 是否应当总是 resolve schema，还是只在单边 bare / applied 不对称时 resolve？
2. `CalcitEnum` 的泛型名字应当直接存放在 enum 定义上，还是通过原始 record/schema 间接恢复？
3. bindings 更完整后，是否需要把某些当前依赖 `dynamic` 的 core helper 一并收紧？

## 结论

建议继续沿“Rust 风格的统一规则”推进命名类型泛型绑定，但要保持增量落地：

- 先补统一逻辑
- 再观察新增 warning 面
- 最后才决定是否提高错误等级

本轮 `Struct <-> TypeRef` 的修复已经证明这条方向风险可控，而且能直接提升泛型返回特化和 `:where` 检查的可靠性。下一步最值得做的是 `TypeRef <-> TypeRef` 的受控绑定，以及 enum 元数据补齐。