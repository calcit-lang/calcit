# RFC：`defimpl Type Trait` 与 `Self` 的收敛

状态：Accepted（范围已确认，待实现）

日期：2026-11-16

关联：

- [#2727](https://github.com/calcit-lang/calcit/issues/2727)（本 RFC 的主 issue）
- [#2704](https://github.com/calcit-lang/calcit/issues/2704)、[#2696](https://github.com/calcit-lang/calcit/issues/2696)（trait 写进类型位置的退役）
- [#2736](https://github.com/calcit-lang/calcit/issues/2736)（按接收者选签名改为 `:cases` 数据）
- [#2837](https://github.com/calcit-lang/calcit/pull/2837)、[#2843](https://github.com/calcit-lang/calcit/pull/2843)（`Self` 双向推导、defimpl 别名与局部名义闭环）
- [RFC：trait 的 requires 声明](10-03-trait-requires-rfc.md)
- `docs/features/traits.md`、`docs/run/upgrade.md`

## 摘要

`defimpl Type Trait` 与 `Self` 已在 0.41–0.43 落地大半：顶层 `(:impls …)` 挂载、`deftrait` 的隐式 `Self`、
trait 写进类型位置报 `E_TRAIT_AS_TYPE`、`defimpl ImplName Trait` 旧名退役都已生效。

本 RFC 只做一处语义收敛：**实现方法显式标注的 `hint-fn` 里，`Self` 直接绑定为接收者类型，不再需要 `(:generics Self)`。**
其余登记方式保持现状并作为确定边界：

- `(:impls …)` 保留为显式能力声明，不做自动发现，避免歧义；
- 每个 `defimpl` 仍是必须显式起名的顶层定义；
- 实现方法仍由用户显式标注签名，编译器不从 trait 声明推导实现体；
- 局部 `impl-traits` 的能力证据只限当前函数体。

## 现状

已实现、本 RFC 不改动的部分：

| 能力 | 位置 |
| --- | --- |
| `deftrait` 方法签名用 `Self`，无需 `(:generics Self)`；任一方法用 `Self` 后所有方法首参必须是 `Self`，否则 `E_TRAIT_SELF_RECEIVER` | `src/runner/preprocess/trait_type_names.rs` |
| `deftrait` 自己签名里写 trait 名报 `E_TRAIT_AS_TYPE`，提示改 `Self` | 同上 |
| 顶层 `defimpl Type Trait …` 支持内联方法体与引用已有函数两种写法 | `calcit/test-traits.cirru` 的 `AliasFirstImpl`、`CoreShowImpl` |
| 顶层 `(:impls Trait …)` 只在类型 ns 与 trait ns 找对应 `defimpl`；缺失、重复、未知分别报 `E_MISSING_TRAIT_IMPL`、`E_DUPLICATE_TRAIT_IMPL`、`E_UNKNOWN_IMPLS_TRAIT` | `src/program/impl_attach.rs` |
| `defimpl ImplName Trait` 报 `E_LEGACY_DEFIMPL_NAME`；tag 形式报 `E_LEGACY_DEFIMPL_TAG`；顶层 `impl-traits` 报 `E_LEGACY_IMPL_TRAITS` | `src/runner/preprocess/mod.rs`、`src/bin/cr_tests/type_fail.rs` |
| 同名方法来自不同 trait 来源时 `.method` 报 `E_AMBIGUOUS_TRAIT_METHOD`，需 `&trait-call` 显式选择 | 同上 |
| `requires` 的声明检查与挂载检查 | `RFCs/10-03-trait-requires-rfc.md` |
| 局部类型用 `impl-traits` 挂载；挂载后的静态类型仍等于基础类型 | `docs/features/traits.md` |

实现侧仍存在的不对称（本 RFC 的唯一改动点）：

- 顶层实现的方法体写 `hint-fn $ Fn Self (-> String)` 时，`Self` 仍按普通泛型处理，测试与实现里都需要额外写 `(:generics Self)`；
- 而 `deftrait` 的同一签名不需要 `(:generics Self)`。

## 已确认的设计决策

1. **保留 `(:impls …)`**：类型的能力必须显式列出，编译器不根据 `defimpl` 自动发现，避免“看似存在但未声明”的歧义。
2. **`defimpl` 必须显式起名**：每个实现仍是一个有名字的顶层定义，键进入 Snapshot；不做派生键或匿名实现。
3. `(:impls …)` 与 `defimpl` 双处登记保持现状：一处声明能力，一处提供实现，缺失或重复按现有诊断处理。
4. 实现方法**仍必须由用户显式标注签名**；不从 trait 声明推导实现体类型，未标注时保持现有动态方法语义。
5. 局部 `impl-traits` 的能力证据只在当前函数体内有效，跨返回值不传播，不引入第二种名义身份。
6. 迁移以诊断提示引导，按提示手工修改；不新增 `calcit fix` 固定改写规则。

## 表层写法

### 顶层

```cirru.no-check
; app.shape
deftrait Show
  .show $ Fn Self (-> String)

defstruct Circle (:r Number) (:impls Show)

; 顶层定义名（Snapshot 键）由用户提供：CircleShowImpl
defimpl Circle Show
  .show $ fn (self)
    hint-fn $ Fn Self (-> String)
    str |circle: $ :r self
```

- 类型在 `(:impls …)` 中声明能力，`defimpl` 提供实现，两处必须一致。
- `self` 的类型就是 `Circle`；`hint-fn` 里的 `Self` 绑定为 `Circle`，不需要 `(:generics Self)`。
- 实现方法仍必须显式标注；不写 `hint-fn` 时不获得 trait 声明带来的静态证明，按现有动态方法语义处理。
- `defimpl` 所在的顶层定义名（`CircleShowImpl`）继续由用户提供，键进入 Snapshot。

### 局部类型

```cirru.no-check
let
    Box0 $ defstruct Box (:value Number)
    BoxTraitImpl $ defimpl Box0 BoxTrait
      .read $ fn (self)
        hint-fn $ Fn Self (-> Number)
        :value self
    Box $ impl-traits Box0 BoxTraitImpl
  (Box :value 1) .read
```

`impl-traits` 只用于 `let` 内局部类型，顶层继续报 `E_LEGACY_IMPL_TRAITS`。同一函数体内，挂载后的 `Box` 携带 impl 证据；返回给调用方时仍以基础类型 `Box0` 的名义身份为准。

## 语义

### 挂载仍显式声明

- 顶层类型的能力集合 = `(:impls …)` 列出的 trait；实现按现有规则在类型 ns 与 trait ns 查找。
- 未在 `(:impls …)` 列出的 trait，即使旁边存在 `defimpl` 也不会挂载（现状不变）。
- 列出但找不到实现报 `E_MISSING_TRAIT_IMPL`；同一 trait 两个实现报 `E_DUPLICATE_TRAIT_IMPL`；名字解析不到报 `E_UNKNOWN_IMPLS_TRAIT`。
- 不新增自动发现路径，也不改变跨 ns 查找规则。

### `Self` 在实现体内隐式绑定

- `defimpl Type Trait` 内，方法体第一个参数的类型为 `Type`；`hint-fn` 或方法值上的类型合同里，`Self` 绑定为 `Type`，不需要 `(:generics Self)`。
- `Self` 出现在参数、返回值和容器（`(List Self)` 等）时，统一替换为 `Type`。
- 实现方法仍由用户显式标注签名。编译器不从 trait 声明推导实现体签名，也不为未标注的实现体生成静态证明。
- 未标注签名的实现保持现有动态方法语义；trait 声明本身是 `'Fn`、没有完整合同时同样如此。
- 已标注的签名与 trait 声明不一致时沿用现有实现检查诊断（如 `E_METHOD_ARG_TYPE_MISMATCH`），不因 `Self` 的隐式绑定而放宽。
- `Self` 在其他 schema（普通 `defn`、`hint-fn`、`assert-type`）里仍报 `E_UNKNOWN_TYPE_NAME`，本 RFC 不扩大作用域。

### 局部 `impl-traits` 的证据边界

- 挂载结果在同一函数体内可用作 `.method` 与 `assert-traits` 的静态证据。
- 返回值只保留基础类型的名义身份；接收方需要能力时必须在自己的作用域内重新挂载，或通过 `:where` 约束声明。
- 不引入隐式全局实现搜索，也不把局部挂载写入 Snapshot。

### 分派与冲突

- 同一方法名来自不同 nominal trait 来源：`.method` 报 `E_AMBIGUOUS_TRAIT_METHOD`，用 `&trait-call Trait :method receiver` 明确来源。
- 同一来源的重复实现：`E_DUPLICATE_TRAIT_IMPL`，运行时不按挂载顺序选择。
- trait 描述值身份仍是名义的运行时身份；重新求值 trait 定义产生新身份，旧 impl 不自动满足新 trait。

## 诊断

| 情形 | 诊断 | 变化 |
| --- | --- | --- |
| 两个 `defimpl Type Trait` 指向同一 trait | `E_DUPLICATE_TRAIT_IMPL` | 复用 |
| `(:impls …)` 列了 trait 却没有实现 | `E_MISSING_TRAIT_IMPL` | 复用 |
| `(:impls …)` 名字解析不到 | `E_UNKNOWN_IMPLS_TRAIT` | 复用 |
| 两个 trait 提供同名方法后直接 `.method` | `E_AMBIGUOUS_TRAIT_METHOD` | 复用 |
| 已标注实现签名与 trait 声明不符 | 现有实现检查诊断（`E_METHOD_ARG_TYPE_MISMATCH` 等） | 复用 |
| 普通 trait 出现在类型位置 | `E_TRAIT_AS_TYPE` | 复用 |
| `defimpl ImplName Trait`、tag 形式、顶层 `impl-traits` | `E_LEGACY_DEFIMPL_NAME` / `E_LEGACY_DEFIMPL_TAG` / `E_LEGACY_IMPL_TRAITS` | 复用 |

不新增诊断编号：本条 RFC 只改变 `Self` 的绑定方式，不引入新的失败类别。

## 正反规格（实现时写成 `:tests`）

| 用例 | 期望 |
| --- | --- |
| 顶层 `(:impls Show)` + 具名 `defimpl`，调用 `.show` | 通过（现状） |
| `(:impls Show)` 但旁边有未列出的 trait 实现 | 不挂载，调用报缺少实现（现状） |
| 两个 `defimpl Circle Show` | `E_DUPLICATE_TRAIT_IMPL` |
| `(:impls Show)` 但无 `defimpl` | `E_MISSING_TRAIT_IMPL` |
| impl 内 `hint-fn $ Fn Self (-> String)`（不写 `(:generics Self)`） | 通过，`Self` 按 `Circle` 解释 |
| impl 内 `hint-fn $ Fn Self (-> Self)` | 通过，返回类型为 `Circle` |
| impl 内 `hint-fn $ Fn Self (-> (List Self))` | 通过，`List Circle` |
| impl 内继续写 `(:generics Self)` | 继续接受，不报错 |
| 未标注签名的实现方法 | 保持现有动态方法语义，不获得 `Self` 的静态证明 |
| 普通 `defn` 的 `hint-fn` 里写 `Self` | `E_UNKNOWN_TYPE_NAME` |
| `Show` 与 `Labeled` 都声明 `.label`，同一类型都实现后直接 `.label` | `E_AMBIGUOUS_TRAIT_METHOD` |
| 上例改用 `&trait-call Show :label x` | 通过，返回 `String` |
| 局部 `impl-traits` 后在同一 `let` 内调用方法 | 通过 |
| 把挂载后的局部值按基础类型 schema 返回，调用方调用方法 | 报静态证据缺失，提示在调用方重新挂载或加 `:where` |
| `deftrait` 方法签名写自身 trait 名 | `E_TRAIT_AS_TYPE`，提示 `Self` |
| `requires` 子 trait 缺父实现时挂载 | `E_IMPL_MISSING_REQUIRED_TRAIT` |

## 后端范围

- native 与 JS：完整支持，包含 `Self` 隐式绑定的静态检查。
- WASM：沿用现有边界，只编译能静态确定接收者的方法调用；运行时 `assert-traits` 继续明确报 unsupported。
- 本 RFC 不改变运行时 impl 表示，只改变已标注签名中 `Self` 的绑定。

## 兼容与迁移

- 纯增量：现有写 `(:generics Self)` 的实现继续有效；省略后行为一致。
- `(:impls …)`、`defimpl` 名字、跨 ns 查找、动态方法语义均不变，不需要迁移现有 Snapshot。
- 迁移以诊断提示为主：报错信息给出位置与目标写法，用户按提示手工修改；不新增 `calcit fix` 固定改写规则。
- 生态 11 处实现（alerts 6、cumulo-util 3、memof 1、recollect 1）不需要改动。
- 现有 `E_LEGACY_*` 退役项保持，不恢复旧入口。

## 非目标

- 把 `(:impls …)` 改为可选、自动发现或删除。
- 允许匿名 `defimpl` 或派生定义键。
- 由 trait 声明自动推导实现体签名。
- 局部挂载能力跨函数传播；不引入隐式全局实现搜索或第二种名义身份。
- trait 默认方法、associated types、blanket impl、specialization。
- `Eq` / `Hash` 等 core trait 关系调整。
- `analyze weak-types` 或其它迁移统计规则扩展。

## 实施步骤

1. 在 `defimpl` 方法的签名解析中把 `Self` 绑定为接收者类型；已标注的 `hint-fn` 不写 `(:generics Self)` 时按该绑定检查，写了继续接受。
2. 保持 `Self` 在其他 schema 中的 `E_UNKNOWN_TYPE_NAME` 行为，补充正反测试（`Fn Self ...`、`(List Self)`、嵌套回调）。
3. 更新 `docs/features/traits.md` 的 `defimpl` 示例与限制说明；`docs/run/upgrade.md` 不需要新增迁移条目（纯增量）。
4. 在 `calcit/test-traits.cirru` 增加 definition `:tests`，覆盖上表与「`(:impls)` 未列出不挂载」的现状。
5. 用 Recollect、Alerts、Memof、Cumulo Util 的现有迁移分支做回归，确认不需要改消费者源码。

## 实现验收

- `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test` 通过。
- `CALCIT_PROOF_AUDIT=0 CALCIT_LINT_CORE=1 yarn check-all` 通过。
- 新增测试在 native 与 JS 共用运行器上语义一致。
