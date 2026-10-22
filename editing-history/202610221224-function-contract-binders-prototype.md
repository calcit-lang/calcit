# 函数合同的词法泛型对应

本记录对应 #2190；相关的旧 kind→T 许可及其他 core 合同仍由 #2187 / #2121 跟踪。
这是当前分支的设计和验证方式，不代表已合并、已发版或完成所有类型系统任务。

## 问题与统一关系

同一函数的 CodeEntry 使用 C、部分 body Map 使用 T 时，原实现继承的返回值、rest 或
where 仍可能引用另一份变量。合法恒等函数和普通 Countable 方法因纯改名被拒绝；
只替换最终签名还会让嵌套回调再次丢失捕获关系。

- Source schema 保留原始数据表示，仅在检查函数体时为 CodeEntry 的量词分配词法身份。
- 从显式重叠的 args、return、rest 用已有 CallTypeProof 证明绑定双射，再继承省略字段。
  不按变量名字、声明顺序或相同 trait 猜测，不把具体类型映成任意泛型。
- 复用 ScopeTypes 的 source-name → binder 映射保留已证明的 body 拼写别名。
  独立内层量词仍遮蔽外层同名变量；没有重新声明量词的回调才捕获外层变量。
- Post-lowering 从已有 checked hint 与原 Map 恢复对应，不增加 Snapshot 字段或另一套证明。
- 枚举 payload 在枚举自身量词的作用域解析，随后实例化；不能被调用方同名别名捕获。
- 实参推断与 where 名称必须来自同一份 callable 合同，不能混用 public 名字和 checked body ID。

Checked contract 返回值只导出实际输入中捕获的词法变量，不导出未绑定输出占位符，
也不从输入函数自己的量词冒充捕获。这替代了按 core 函数名允许泛型结果的分支。
回调输出独立推断；若与调用上下文存在确定的返回形状冲突，仍在实际返回处检查，
保留原 W_FN_RETURN_TYPE_MISMATCH，而不把期望输出当成实现证据。

## Core 的真实合同

扩大词法身份后暴露的合法代码缺口不能靠跳过 core 或扩大 Dynamic 消除：

- deftrait / defenum / defimpl 在构造 trait 自身时，使用已有的基础 List/String 语法树操作，
  避免经通用 map/filter/count 回到尚未建立的 Mappable 等实现；保留宏 DSL、求值与错误顺序。
- Shortcut fold 复用普通 fold 的输入特化，回调输入约束初值，默认值和实际 payload 仍需刚性证明。
  find-last 明确 Option<T>/T 的 accumulator 输入，算法及原断言不变。
- Map filter 直接 fold 独立 key/value，避免经过开放 pair 丢掉 K/V；补测原 pair 顺序、一次求值和空 Map。
- select-keys 与 zipmap 的内部回调/循环声明 Map<K,V> 不变量；空初值、每个写入和 recur 都须符合它。
  保留缺失键跳过、较短输入截断、重复键覆盖等原语义，不要求应用改成 native call。
- 按维护者已选择的方案提供 Emptyable。方法使用同一个接收者泛型 Self，前缀 empty 要求 T:Emptyable
  并返回 T；Self 是普通泛型名，不新增关键字或类型表示。Number/String/List/Map/Set 的既有空值不变，
  容器成员合同保留；自定义名义类型依实际 trait 实现取得能力。Bootstrap 表与真实 core impl 同步。

## 验证与兼容边界

用户可观察语义放在 definition :tests：部分合同、继承能力、嵌套捕获、交叉改名、rest、
Number/String/List/Option payload、Some(None)、Map 算法及自定义 Emptyable 接收者。
源码均通过官方 query/tree/edit、dry-run 与 scoped revision 修改，原断言没有删除或放宽。

编译拒绝扩展原 check-known-assertion harness：T/U 错误捕获、CodeEntry C/body T、
错误 shortcut payload、缺失 Emptyable 能力及伪造接收者身份。保留实际 source 坐标、
无内部 ID、Snapshot 不变与不生成错误产物的检查；合法定义还通过 query/edit 导出后完整 strict。
Rust 只补私有 binder/构造器/输出占位符等内部 invariant，既有拒绝诊断断言保持原样。

完整 core、函数合同的 native/JS 回放、Rust 库与编译拒绝、原全量断言 harness、默认严格检查、
Agent 接口、历史 API 合同、TS 编译及相关真实项目验证仍分别执行。部分 gate 通过不冒充最新候选全绿，
WASM/WASI 只按原运行器与已支持用例验证，不宣称支持任意泛型闭包或 first-class type definition。

没有扩大 Dynamic、加入 unsafe、增加 checker/诊断编号/计数门禁，也没有批准旧 API 删除。
升级说明解释 Emptyable 约束和 body 合同对应；具体提交与 CI 证据集中记录在最终 PR。
