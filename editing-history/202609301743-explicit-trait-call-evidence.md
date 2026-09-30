# 显式 trait 调用的确定证据边界

在 main `5420a34b525ed71d0cd9723097073edabe4c3e3a` 上，`&trait-call calcit.core/Countable :count 1` 的默认严格预处理通过，运行时才因 Number 没有 Countable 实现失败。手写显式 trait 名称只解决不同 trait 同名歧义，不能作为 receiver 已实现该 trait 的证明。

本次仅把可确定的证据接入已有预处理：解析名义 trait/method，复用方法 schema 做参数个数检查，读取现有 receiver impl 元数据，并拒绝内建封闭类型上的确定缺失或重复实现。用户定义的局部 `impl-traits` 可附着到 Struct/Enum 的本地别名，而现有类型推断有时只保留原始 nominal 类型；具名泛型的 `:where` 也未直接保存在每个参数的类型标注中。因此不能把从该类型查不到 impl 或看到单独的 TypeVar 直接当成缺失。保留运行时校验与既有成功/失败语义，不生成新的转换或第二套派发协议。

Calcit definition `:tests` 覆盖成功调用；Rust 负例覆盖严格预处理诊断。#1531 尚需在后续阶段让参数类型、返回值、宏生成 IR 和局部 impl 附着来源共享完整 checked-call 证明；本次不是该 issue 的完成声明。
