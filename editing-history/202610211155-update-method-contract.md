# update 内部迁移到方法

## 设计依据

按 #2187 已确认的方向，先迁移旧 assoc/dissoc 的真实调用，再受保护地退场旧入口。本步只迁移 update 内部的 assoc 调用；不删除公共入口，不修改 schema，不引入新的类型证明规则。

Map/List 通过既有 .assoc 方法执行。开放 Enum 只有 kind 证据，没有名义方法 origin，因此沿用旧 assoc 实际调用的 &enum:assoc；不为它新增 originless-method 豁免。

开放 Struct 保留原接收者的运行时 .assoc 派发，并保留旧 assoc 的 Struct 返回值检查。直接调用方法而省去该检查会接受自定义实现返回的 String，属于真实语义回退。回归测试断言精确错误，以及 callback、method 各一次且依次执行。

## 兼容边界与验证

- 原 update 的五项 :tests 保持不变；使用现有统一运行器回放 native/JS。
- 新增定义 :tests 放在现有 trait-bound-return fixture，通过原校验脚本回放 native/JS。它使用局部 trait/Struct 和 try，不宣称 WASM 已支持这些构造，也不新增排除项。
- 不扩大 Dynamic、不新增 unsafe，不宣称已完成公共 assoc/dissoc 退场或完整类型原型验收。
