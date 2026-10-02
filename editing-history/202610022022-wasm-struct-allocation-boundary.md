# WASM Struct 字段更新的分配边界

## 实际缺口

普通 `.assoc` lowering 最终调用静态字段更新 emitter。旧实现只比较 receiver payload 中的 nominal layout ID，然后按 receiver count 复制并建立 Struct header。raw WASM export 的宿主可以传入真实 List pointer；首项等于合法 layout ID 时旧实现未 trap，已有 Node runner 的新增负例实际复现了这一错误对象类型接受路径。

这证明生成模块内的名义分配类型校验不足，不表示宿主执行权限被突破。正常源码方法调用、Struct schema 和各 backend 的共享语义保持不变。

## 修复决策

复用既有 `emit_type_of_local` 的 heap range、magic 和 allocation kind 检查，不重新建立一套 runtime type 分类。receiver 与更新值各求值一次，再检查 Struct header、8 字节对齐和可读取的 count/layout prefix。

选中实际注册布局时同时要求 count 恰好等于字段数。复制 slots 从这个注册布局计算，编译期检查乘法范围，运行时检查整个复制 span 不越过当前 allocated heap；所有拒绝在结果分配和复制之前发生。保留实际 nominal identity、原值及其他字段。

检查基于现有内部 heap header 和注册布局，不新增 allocator registry，也不承诺抵御宿主任意重写整个线性内存的能力。raw WASM f64 pointer 不是经过 Component lifting 的受检宿主值，调用方不能把源码 schema 当成 raw export 参数的运行时验证。

## 验证

沿用 definition-attached `write-count` 的正常 `.assoc` 语义测试，在 native/JS/WASM 验证原值、其他字段和 nominal identity；不改成 native call。

特殊内存状态不能通过稳定的正常 Calcit 构造表达，故在既有 runner 中通过 typed raw exports 和 DataView 做 backend 专属低层探针：真实 List 的匹配 layout 槽、负数/零/短/长/小数/NaN/Infinity count、错误 magic、非法 pointer、越过 heap 的 prefix/span 和过期 index/tag。错误 count 与 heap span 的探针同时检查拒绝前没有分配结果；错误更新不覆盖源字段。完整 runner 的既有证明、decode、alias 和其他 backend 回归继续执行。

验证命令、最新 CI 和 review 证据放在 PR 中，尚未完成的门禁不写成已交付。
