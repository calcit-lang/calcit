# Struct 数据字段写入合同

## 原因

Diary 受检 patch 迁移暴露动态 Struct 写入的证明缺口（#1868）。进一步回放发现，旧 native/JS matcher 只看 List/Map/Set 外层 kind，并可能按短名接受来自另一命名空间的 Struct；JS assoc 遇到缺字段还会 silent no-op。这些已发布行为不足以支持扩大 runtime-checked 白名单。

## 本步边界

先修正可解析数据合同的运行时行为，不新增公开命令或检查规则，不放宽 strict。native 写入复用 DataShapeGraph，JS 编译器携带同一图的字段校验函数；JS validator 只检查原值，不使用 decoder 的 Map 转换或 Option lifting。标量沿用原轻量检查。所有写入与 indexed lowering 共用字段检查，失败不修改原值。

名义定义的别名必须保留原声明身份：静态查找支持 source 中尚未 lower 的完整限定引用；JS alias binding 不重绑 origin，impl 装饰保留字段 validator。泛型字段有具体实参时复用现有 substitution；没有重建或猜测 erased receiver 的泛型实参。

## 尚未完成

无法建立 data shape 的字段仍沿用原运行时检查，不将其宣传为完整证明。任意 Dynamic receiver、未实例化泛型、函数/host 合同以及局部动态 prototype 仍需后续统一解决；尤其现有 `&struct:with` 的宽泛 runtime-checked 登记不能作为扩大其他入口的依据。#1868、Recollect #76 与 Diary 完整 strict 的发布验收仍保持开放。本步不是这些任务全部完成，也不包含发版。

## 验证

在已有 def-value-schema fixture 使用 Calcit CLI 的 dry-run/revision transaction 添加 definition `:tests`，既有 check-struct-field-js 与统一 runner 回放。覆盖四个更新入口、动态 Tag/String key（indexed 入口依原合同使用 Tag）、缺字段、嵌套 Map/List/Optional、Set、nominal origin、别名、具体泛型、Enum payload、空容器、显式 Dynamic 叶子及原值不可变。保留原静态拒绝、跨后端与真实消费者门禁。
