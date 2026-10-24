# 按真实 callable 身份合并 core 兼容重复项

## 问题与选择

真实消费者用 Countable 的 `count` 检查开放 Enum，查询却把 `.count` 的 core nominal impl
与旧 inherent bag 视作歧义。隔离项目进一步确认：同一个 `Fn [Enum] -> Number` 参数调用
`count` 可以通过，改为 `.count` 则被既有歧义诊断拒绝。这不是应该增加固定迁移规则的问题。

复用共享方法解析，不只改查询状态。保留原候选顺序，仅在优先候选是实际 core 表中的 nominal
impl、其余全部是同一 core 表中的 inherent 兼容项，并且指向同一个 callable 时合并。
core 身份比较已有的字段/值存储身份，并保留 origin 的身份；不信短名称、相等内容或相似函数体。
现有 nominal 合同仍由原绑定、feature 与返回推断路径处理。

另一个 nominal origin、重复 nominal impl、用户构造的同名 bag 或不同 callable 继续歧义。
Enum 仍按原来的 last-wins 顺序取候选；没有把它加入 first-wins 表，也不额外开放其他 originless
Enum 方法。Struct 的实际 core 身份检查复用同一个内部 helper。

`.contains?` 的两份 core 代码虽相似，却是两个不同函数值，本次不猜测它们等价；开放 Enum
的 payload 也不会因此变成已知类型。不新增 API、checker、诊断编号、固定改写、Dynamic 或 unsafe。

## 验证放置

- 用户语义在原 trait fixture 的 `:tests`：包含 tag 的三项/四项 Enum，同时调用显式 Enum
  参数的 helper 和具体名义值的方法。原有定义与断言保持，使用官方事务、dry-run、revision
  守卫和完整查询读回添加。
- 同一运行器的既有 ResultBox portable 批次增加该 target，不新建脚本或 CI job；原 native/JS
  trait 全集也自动包含新测试。WASI 可通过既有 opt-in backend 验证。
- 原查询测试校验 Countable 来源、零个方法参数、Number 返回及 primitive 定义定位，保留不同
  callable 的歧义与开放 payload。内部伪造 core bag、不同/重复 nominal origin 的条件留在原
  Rust 模块测试，避免给语言新增制造历史宿主状态的接口。
- 旧 CLI 的语言回放与旧实现的查询测试均先复现错误；最终回放、严格检查、review、CI 与精确
  main 状态以关联 PR 的实际记录为准，不用本历史文件代替验收。
