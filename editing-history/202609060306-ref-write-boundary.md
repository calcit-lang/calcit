# Ref write boundary / 可变引用写入边界

Issue: https://github.com/calcit-lang/calcit/issues/879

Current handoff: the checkpoints below are chronological. Public issue creation
was subsequently explicitly approved by the user and #879 is now in milestone 2.
The generic, outer-unknown and nested-constructor gaps below have been addressed.
Calcium integration with separately owned local ws-edn/cumulo-util fixes passes
client/server preprocessing, all 23 native tests, generated-JS projection and
serialized snapshot/patch regressions, production build and browser style smoke.
Those provider fixes are unreleased; formal dependencies remain unchanged, and
whole strict client/server still stop at cursor Map / parsed-database boundaries.
The candidate binary was invoked explicitly; no global toolchain was replaced.
Reel remains owned by other PRs. No merge or release is claimed by this record.

用户批准后已公开创建 #879；下文保留早期失败和逐步修复记录。真实应用本地联调
通过，但未发布 provider、正式依赖和全项目 strict 验收仍需单独推进。

During Calcium strict migration, a full-state assertion passed while a literal
get-in traversal returned none. A dependency-free reproduction on published
0.13.77 and unreleased main 74420e5d confirms the difference only after mutating
an atom to an incompatible nested Map payload. An immutable Map works.

Atom inference retains Ref<T> from its initializer and deref restores T. Reset
previously only preprocessed arguments, so subsequent typed reads could trust
evidence invalidated by a write. The local candidate routes reset! through the
existing argument checker with the target Ref's payload as its expected type.
It does not widen declared types or disable literal-path optimization.

Regressions cover scalar and nested Map mismatches, a non-Ref target, compatible
scalar/Map writes, declared Ref<Number> acceptance/rejection, and an explicit
Ref<Dynamic> boundary. The initial full cargo test run passed; subsequently
added declared/open-reference cases passed in a focused rerun. cargo fmt --check,
git diff --check and all-target clippy with -D warnings pass. Generated-JS and
application validation, plus a final full run on the exact final patch, remain
required; this checkpoint is not a release or merge claim.

在真实应用回归中发现引用写入缺少校验，已缩小为无依赖合成复现。修复候选复用
现有参数检查器，禁止不兼容写入破坏后续类型化读取的依据，不通过降低类型要求
或关闭优化规避问题。Reel 及其工作区未修改。

An issue draft is local under .calcit/snippets/atom-reset-issue.md. Public issue
creation was rejected by permission review; no issue was created, and publishing
this diagnostic requires explicit user approval. Do not retry indirectly.

## Consumer and full-suite checkpoint

The exact current patch passes cargo test (712 library, 301 CLI, 23 integration)
and Node 24 yarn check-all, including Agent interface 18/18, native/JS suites,
WASM, literal-path and typed-method checks. Calcium server preprocessing passes.
Calcium client preprocessing now reports three genuine boundary disagreements:
two ws-edn timer-ref writes claim Option<Number> but receive Option<JsNullish<JsObject>>;
cumulo-util.activity's touch timer claims Number but receives JsNullish<JsObject>.
These are not suppressed and no installed provider was edited.

A further synthetic probe remains accepted and prevents treating this candidate
as complete: a function with args Ref<T>, T can reset its reference to a fixed
String. The shared generic argument matcher specializes T at the write instead
of treating the reference's T as fixed. Next work must distinguish a reference's
existing generic contract from fresh call-site inference, test accepted same-T
writes and rejected unrelated writes, and audit Dynamic-to-concrete writes.

完整检查通过但泛型引用反例仍未阻止，因此当前候选不能合并或发版。真实应用
新增的三个 timer 边界告警保留，后续需由其所属模块补齐真实宿主类型契约。

## Rigid generic write follow-up

The generic counterexample above is now rejected. The write check uses a fresh
binding probe and rejects writes that would specialize existing type variables.
Regressions reject String into Ref<T>, unrelated T into Ref<Number>, and List<U>
into Ref<List<T>>; same-T scalar and List writes continue to pass. Full Rust
tests pass (712/301/23). Clippy's needless closure borrow was fixed, not allowed
away; clippy and check-all are being rerun on that exact patch.

Dynamic-to-concrete Ref writes still require a focused strict fixture. A direct
strict exec probe stopped earlier at its synthetic entry's undeclared schema,
so it supplies no evidence about that write boundary. Do not treat this gap as
resolved or publish/merge the current candidate yet.

泛型反例已补齐正反回归，拒绝写入时重新绑定已有类型参数；全量 Rust 测试通过。
仍需单独验证 Dynamic 写入边界，不能把提前失败的 strict 入口当作验收证据。

The generic follow-up's clippy and full check-all reruns completed successfully.
A subsequent strict probe adds an explicit Unit entry hint: main preprocessing
accepts a local `(Ref<Number>, Dynamic) -> Unit` function that resets the reference
from that Dynamic argument; the default synthetic reload entry then fails its
unrelated missing-schema gate. This narrows the next investigation to unknown
payload writes, not the already-fixed generic specialization case.

## Strict unknown outer payload

Strict project writes now reject whole Dynamic/DynFn values (including absent
type evidence) when the Ref payload has a non-Dynamic contract. Ref<Dynamic>
continues to accept unknown values intentionally; compatibility mode is unchanged.
The new CLI regression assigns both init and reload to the hinted entry, so an
unrelated missing-schema error cannot satisfy the negative test. It checks
Ref<Number>, Ref<List<Dynamic>>, Ref<T> rejection and Ref<Dynamic> acceptance.
All four cases pass. The first red test exposed that resolve_type_value returns
None for some Dynamic locals; the checker now treats that as unknown rather than
silently skipping the write check.

This checks the outer payload contract; nested erased positions in otherwise
known containers and empty-container/none exceptions still need an explicit
audit before calling the full Ref boundary complete.

严格模式未知值写入已补回归：具体外层契约不能被未知值覆盖，显式开放引用仍可用。
嵌套容器的未知位置及空值构造例外尚待审计，不提前宣称全部边界完成。

The strict-outer follow-up passes cargo fmt --check, git diff --check and the
full cargo test run (712 library, 302 CLI, 23 integration tests). Final check-all
must be rerun after completing the remaining nested-position audit; the prior
check-all success covers the generic-write checkpoint, not this later patch.

## Nested erased positions and constructor evidence

Strict reset checks now compare nested type positions against the fixed target
contract, including collection keys/values, nominal arguments, and function
signatures. Explicit Dynamic positions remain open. Literal List/Set/Map and
core Option constructors provide value-level evidence, allowing empty and nested
empty literals without pretending their inferred element types are concrete.
Literal elements are checked individually, so wrapping an unknown value in a
List does not bypass the boundary.

The new ten-case strict regression passes: reject List<Dynamic>, Map<Tag,Dynamic>
and Option<Dynamic> into concrete payloads; preserve open List<Dynamic>; accept
empty List/Set/Map, nested empty List and %none; reject a literal List containing
an unknown element. Full Rust tests (712/303/23), formatting and all-target clippy
pass. Full check-all is running on this nested-position checkpoint.

嵌套未知位置已增加定向检查和十组正反回归；空构造器使用实际表达式证据，
不通过扩大目标类型或关闭门禁放行。当前仍未公开发布、合并或发版。

The nested-position full check-all run completed successfully. Calcium's full
23-test selection with the new compiler is **not green**: 22 pass, while the
dispatch regression is blocked during preprocessing by the three timer warnings
and an additional ws-edn global-client write (Option<WsClient> versus Option<T>).
That fourth diagnostic needs source-level validation before changing its provider
or calling it a confirmed provider defect. No warning was suppressed.
