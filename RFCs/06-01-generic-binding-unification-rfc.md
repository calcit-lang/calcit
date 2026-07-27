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

基于当前版本，底层状态已经比最初起草这份 RFC 时更进一步：

- `CalcitStruct.generics` 已参与 `Struct <-> Struct` 与 `Struct <-> TypeRef` 的单边 applied 绑定
- `CalcitEnum` 现在也已经保存 `generics` 元数据
- `&enum::new` 已支持泛型参数列表，enum payload 里的类型变量可以稳定保留下来
- `defstruct Box ([] 'T) (:value 'T)` 这一类表层 struct 泛型声明已经能直接运行

这意味着 RFC 的关注点可以收窄成两部分：

- 先把已经具备元数据的路径补成对称行为
- 再处理确实仍需 schema lookup 的 `TypeRef <-> TypeRef`

最明显的缺口是：当两个命名类型本质上表示同一个定义，但只有一侧携带了已应用的泛型参数时，绑定行为并不一致。

- `Struct(Applied)` 对 `Struct(Bare)`：当前已经会把已应用参数绑定回声明的泛型变量。
- `TypeRef(Applied)` 对 `TypeRef(Bare)`：当前基本仍是宽松通过，不一定留下可用绑定。
- `Struct(Applied)` 对 `TypeRef(Bare)`：本轮已经补齐绑定。
- `Enum(Applied)` 对 `TypeRef(Bare)`：在新版之前缺失；现在底层元数据已经具备，可以直接补齐为与 struct 对称的绑定。

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

## 当前已落地能力

截至当前版本，已经确认可用的基础能力包括：

- `Struct <-> TypeRef` 在单边 applied 时会把实参绑定回 `CalcitStruct.generics`
- `Enum` 定义已经持有 `generics` 元数据，不再需要借助旧 record 结构旁敲侧击恢复泛型名
- struct 泛型的表层声明、enum 泛型的运行时构造与 applied named type annotation 都已经能通过文档和 `eval` 验证

其中，`Struct <-> TypeRef` 已经落地的统一行为是：

- 两边都 bare：只检查命名是否一致
- 两边都 applied：逐项匹配参数
- 一边 bare、一边 applied：把 applied 参数绑定回 `CalcitStruct.generics`

也就是从“宽松通过但不留痕”改成“通过且留下绑定”。

enum 侧现在也具备做同等级修复的前提，不再属于“缺底层表示”的阶段。

## 先看 Calcit 里的实际写法

为了避免一直停留在内部 Rust 表示，先把这个问题翻回 Calcit 代码。

当前你最熟悉的两类表面写法大致是：

```cirru
defn id2 (x)
	hint-fn $ {}
		:generics $ [] 'T
		:args $ [] 'T
		:return 'T
	x

defn show-id (x)
	hint-fn $ {}
		:generics $ [] 'T
		:where $ {} ('T Show)
		:args $ [] 'T
		:return :string
	.show x
```

这一层已经很像 Rust：

- `'T` 是泛型变量
- `:where` 约束表示 `'T` 必须满足某个 trait
- 调用点先绑定 `'T`，再拿绑定结果检查 `:where`

而命名类型这边，用户实际写的是：

```cirru
defstruct Pair
	:left :number
	:right :string

defstruct Holder
	:box Pair

defenum Wrapped
	:pair Pair
	:none
```

这里的问题不是 Calcit 没有泛型，而是“命名类型在不同写法之间切换时，绑定信息没有总是被保留下来”。

## 用 Calcit 代码看这个问题

下面几段代码故意把内部 `Struct(...)` / `TypeRef(...)` 还原成用户更关心的表面写法。

### 场景 0：先看一个正常的泛型绑定

```cirru
defn echo-box (x)
	hint-fn $ {}
		:generics $ [] 'T
		:args $ [] (:: Box 'T)
		:return 'T
	get x :value

defstruct Box $ :value 'T

defn demo-ok ()
	let
			b $ %{} Box (:value 1)
			n $ echo-box b
		assert-type n :number
```

你希望编译器在调用 `echo-box b` 时做的事情其实很简单：

1. 从参数 `b` 看出它是 `Box<number>`
2. 把 schema 里的 `'T` 绑定成 `:number`
3. 再把返回类型 `'T` 专门化成 `:number`

这条链路本身没有争议。

真正的分歧出在：如果 `Box 'T` 不是直接出现在同一种内部表示里，而是有时被保留成命名引用，有时已经解成结构定义，那还要不要留下 `'T -> :number` 这组绑定？

### 场景 1：`Struct(Applied)` 对 `TypeRef(Bare)`

对应的用户代码可以想成：

```cirru
defstruct Pair
	:left 'A
	:right 'B

defn keep-pair (p)
	hint-fn $ {}
		:generics $ [] 'A 'B
		:args $ [] Pair
		:return Pair
	p

defn demo-struct-to-named ()
	let
			p $ %{} Pair (:left 1) (:right |hi)
			out $ keep-pair p
		assert-type out Pair
```

这段表面上看不出问题，因为 `assert-type out Pair` 太粗了，只要求它还是 `Pair`。

但内部其实还有一个更细的问题：

- `p` 这一侧已经知道是 `Pair<number, string>`
- `keep-pair` 的参数 schema 另一侧可能只保留成名字 `Pair`

如果这一步只判断“都是 Pair，所以 ok”，那 `'A` / `'B` 实际上没有被绑定下来。

这就会影响后续更依赖精确信息的场景，比如：

```cirru
defn pair-left (p)
	hint-fn $ {}
		:generics $ [] 'A 'B
		:args $ [] Pair
		:return 'A
	get p :left
```

如果前一步没有留下 `'A -> :number`，这里的 `:return 'A` 就更容易退回弱类型。

### 场景 2：`TypeRef(Applied)` 对 `Struct(Bare)`

这个场景在表面代码里更像“类型信息从引用侧来，而不是从结构定义侧来”。

```cirru
defstruct Pair
	:left 'A
	:right 'B

defn pass-through (p)
	hint-fn $ {}
		:generics $ [] 'A 'B
		:args $ [] (:: Pair 'A 'B)
		:return (:: Pair 'A 'B)
	p

defn takes-pair (p)
	hint-fn $ {}
		:args $ [] Pair
		:return Pair
	p

defn demo-named-to-struct ()
	let
			p $ pass-through $ %{} Pair (:left 1) (:right |hi)
			out $ takes-pair p
		assert-type out Pair
```

这里你可以把 `pass-through` 想成“把具体参数挂在名字上”，把 `takes-pair` 想成“只认这个结构定义”。

理论上它们之间没有本质差异，都该把：

- `'A -> :number`
- `'B -> :string`

留给后续链路。

这也是为什么本轮已经先补 `Struct <-> TypeRef`，因为它是最小、最安全、又最接近 Rust 统一行为的一段。

### 场景 3：`TypeRef(Applied)` 对 `TypeRef(Bare)`

这是下一步最值得讨论的点，因为它表面上最“正常”，但内部最容易宽松放过。

```cirru
defstruct Pair
	:left 'A
	:right 'B

defn id-pair (p)
	hint-fn $ {}
		:generics $ [] 'A 'B
		:args $ [] (:: Pair 'A 'B)
		:return (:: Pair 'A 'B)
	p

defn erase-pair (p)
	hint-fn $ {}
		:args $ [] Pair
		:return Pair
	p

defn demo-named-to-named ()
	let
			p $ id-pair $ %{} Pair (:left 1) (:right |hi)
			out $ erase-pair p
		assert-type out Pair
```

这段为什么难判断？

- 用户只看到 `Pair` 和 `(:: Pair 'A 'B)` 都是“同一个名字”
- 但实现上 `TypeRef` 自己并不知道 `Pair` 的第 0 个参数叫 `'A`，第 1 个参数叫 `'B`
- 只有再去 resolve schema，才知道参数位和变量名的对应关系

所以这里的核心不是“要不要更严格”，而是：

- 要不要在这一层做受控 schema lookup
- 做 lookup 后，是不是能稳定拿到 `A/B` 这组声明名

这也是 RFC 里把它单独列成阶段 2，而不是直接和 struct 一起改掉的原因。

### 场景 4：`Enum(Applied)` 对 `TypeRef(Bare)`

enum 侧更容易读懂这个结构性缺口：

```cirru
defenum Result
	:ok 'T
	:err 'E

defn pass-result (x)
	hint-fn $ {}
		:generics $ [] 'T 'E
		:args $ [] Result
		:return Result
	, x

defn demo-result ()
	let
			v $ %:: Result :ok 1
			out $ pass-result v
		assert-type out Result
```

在当前版本里，你期待的绑定已经变成一个可以直接实现的小步，而不是纯设计目标：

- `'T -> :number`
- `'E -> :string`（如果调用点给出的 applied 参数完整）

或者至少在仅一侧 applied 的情况下，把已有那一侧参数回填到 enum 声明的泛型名上。

新版里这块底层元数据已经补齐：`CalcitEnum` 本身就保存 `generics`。因此这里的剩余工作不再是“先改数据结构”，而是把 `matches_with_bindings` 里的 enum 分支改成与 struct 一样的单边绑定策略。

## 为什么这些 Calcit 片段今天还“不够显眼”

如果你只看这些代码，可能会觉得：

- 反正 `assert-type out Pair` 也过了
- 反正 `pass-through` 和 `erase-pair` 都只是原样返回
- 那到底哪里有问题？

关键在于：这里要观察的不是“会不会立刻报错”，而是“后续还能不能继续做精确判断”。

比如把上面的例子继续推进一步：

```cirru
defn pair-left (p)
	hint-fn $ {}
		:generics $ [] 'A 'B
		:args $ [] Pair
		:return 'A
	get p :left

defn show-left (p)
	hint-fn $ {}
		:generics $ [] 'A 'B
		:where $ {} ('A Show)
		:args $ [] Pair
		:return :string
	.show $ pair-left p
```

如果 `Pair<number, string>` 在更早一层只是“名字匹配成功”但没有留下：

- `'A -> :number`
- `'B -> :string`

那么：

- `pair-left` 的返回类型就更容易变弱
- `show-left` 的 `:where ('A Show)` 也更容易看不到真实绑定

所以这个 RFC 讨论的不是“让更多代码报错”，而是“让后续推断不要过早丢信息”。

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

当前版本下的实现前提已经具备：

- `CalcitEnum.generics()` 可以提供 `T/E/...` 这些声明名
- 因此这一步已经下降为“补一段与 struct 对称的匹配代码和测试”

所以 enum 侧现在不再是“缺底层元数据”，而是一个适合直接试做的小步增量。

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

### 2. `TypeRef <-> TypeRef` 仍需要受控 schema lookup

enum 侧的结构升级在当前版本里已经完成，真正还更深的一步反而是 `TypeRef <-> TypeRef`：它要在不引入过度解析和循环依赖的前提下，从名字反查出声明处的泛型变量顺序。

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

### 阶段 2：补齐 `Enum <-> TypeRef` 的单边绑定

建议策略：

- 直接复用 `bind_declared_generics_from_applied_args`
- 行为与 `Struct <-> TypeRef` 保持完全对称
- 先只覆盖“一边 bare、一边 applied”这条路径

这一步风险低，而且能直接验证新版 enum 元数据补齐的实际收益。

### 阶段 3：补齐 `TypeRef(Applied) <-> TypeRef(Bare)`

建议策略：

- 仅在命名可 resolve 到 schema 时启用绑定
- 无法 resolve 时维持当前保守行为

这样不会把整个 `TypeRef` 系统一次性改成强解析。

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
4. `Enum(Applied)` 对 `TypeRef(Bare)` 会留下 bindings
5. 新 bindings 会被 `:where` 检查真实消费，而不是只存在于匹配函数内部

## 开放问题

1. `TypeRef <-> TypeRef` 是否应当总是 resolve schema，还是只在单边 bare / applied 不对称时 resolve？
2. `TypeRef <-> TypeRef` 的 schema lookup 应该缓存到哪一层，才能避免重复解析和循环依赖？
3. bindings 更完整后，是否需要把某些当前依赖 `dynamic` 的 core helper 一并收紧？

## 结论

建议继续沿“Rust 风格的统一规则”推进命名类型泛型绑定，但要保持增量落地：

- 先补统一逻辑
- 再观察新增 warning 面
- 最后才决定是否提高错误等级

`Struct <-> TypeRef` 的修复已经证明这条方向风险可控；而新版 enum 元数据又把 `Enum <-> TypeRef` 从“结构前置缺失”降成了一个直接可做的小步。下一步最值得做的是：先把 enum 单边绑定补齐，验证 warning 面，再推进 `TypeRef <-> TypeRef` 的受控绑定。