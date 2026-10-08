# JS 枚举定义比较与真实消费端边界

Diary 的真实 Respo 分发回归发现，JS 对 `Op` 与 `ClientOp` 的定义值比较返回 true，
native 返回 false。原因是 JS `CalcitEnumDef.name` 为原型方法，旧比较实际比较了同一个函数对象。
这是语言 runtime 缺陷，不应在应用中通过删除负例或放宽业务类型规避。

相等比较复用定义 prototype 的名称、namespace-qualified origin、variant/payload schema
比较，并检查 attached impls。EnumDef 的 hash 仍由 prototype 与 impls 组成；
review 回归进一步确认 impl origin 原先按对象 identity 哈希、按名称 equality 比较，
因此将该哈希键改为 origin 名称文本，保证等价 impl 与 EnumDef 能正常作为集合键。
排序补齐 EnumDef 分支，按 origin、名称、
variant 数量/tag、payload arity/type 和 impls 排序，不再回落到只包含名字的字符串。
不修改普通 enum value 的比较规则，也不新增用户命令或扩大 Dynamic。

用户语义写入 `enum-definition` 的附带测试：不同名称、同名不同 payload、别名、
等价定义的相等/hash，以及 Map/Set 的键行为。复用现有 native/JS/WASM 回放器；
WASM 的 first-class `&enum-def:new` 实际报告 unsupported，保留明确排除原因，
不把本次修复描述为 WASM 已支持这种定义值。

宿主测试补充同名不同 namespace 和 attached impl 的差异、排序反对称、slice/tree 两种 Map。
既有双 runtime fixture 会覆盖 ternary-tree 的进程级 comparator，因此单 runtime 集合检查
放在加载第二份 runtime 之前；后面的跨 runtime impl 身份测试保留。

review 还发现同长度 impl 表的不同函数都可能显示为 `(&fn ...)`，旧排序会错误返回零。
新增 impl 的名称、origin 名称、fields、values 排序；普通函数用 WeakMap 保存进程内
identity 顺序，不写入或强引用函数对象，支持冻结函数。方法闭包仍按方法名排序，
与既有 equality/hash 一致；这种 identity 顺序不宣称跨进程或跨 backend 的具体序号相同。
宿主用例覆盖新建同名 trait origin、同源码不同函数、不同方法值与字段、Map/Set、
反对称和传递性。此处不改变 trait dispatch 选择 origin 的身份规则。

Diary 的修复继续使用实际发布 alpha.19，不替换模块缓存。core 修复仍需走 PR、
最新 HEAD CI/review 与正常预发布，再回到消费者验证，不以本地开发产物冒充正式版本。
