# 保留 map-kv 回调输入的类型关系

`map-kv` 保留旧的二项 List / nil / Enum 回调协议，公共返回仍是显式开放的兼容边界。该兼容性不意味着可以丢掉输入 Map 已有的 K/V 证据。

旧实现通过 `foldl` 把 key/value 装进裸 List，私有 reducer 再用 `&list:nth` 取出；这会把已知的 K/V 变成 Dynamic。普通检查可能通过，但独立证明不能据此证明调用 `Fn<K,V> -> Q` 的输入。修复应发生在真实调用方，而不是给证明关系增加按函数名的豁免。

改为已有 `&map:fold-kv`，让私有 reducer 直接接收 `(acc key value)`，参数注解保留外层 K/V。原 callback 返回值的长度校验、错误文本、nil/Enum 丢弃分支、输出覆盖规则和公共 schema 保持不变；不新增平行 API、Dynamic 或 unsafe。新 typed 代码仍使用已存在的 `filter-map-kv` 与 `MapEntryDecision`，本次不扩大旧入口的使用范围。

语义回归附在定义的 `:tests`：空输入不调用 callback、保留旧丢弃标记、错误传播、非法结果、输入只求值一次、遍历顺序、重复输出 key 最后写入及输入不变。顺序相对于同一后端的原 Map 遍历定义，不要求不同后端的 Map 顺序相同；小/大 Map 经现有 `.assoc` 路径分别覆盖 JS 的切片和树形表示。

证明回归通过官方 query 取得真实定义及 schema，在临时非 core 命名空间检查普通 all-defs、独立 proof 和完整 strict，防止 core 兼容边界掩盖内部证据丢失。使用受保护事务构造临时项目，证明过程不得修改源 Snapshot。跨后端复用现有定义测试运行器；开放旧协议与 try/ref 的后端限制如实记录，不把 native/JS 通过当成 WASM/WASI 已支持。
