# Trait 描述值与实例能力分离

## 依据

`deftrait` 的描述值此前在值推断中直接成为 Trait 能力。于是一个要求 `T: Greeting` 的函数会接受 `Greeting` 本身，普通检查和完整 strict 都通过，native / 实际生成 JS 却在找不到 impl 时失败。反方向上，合法的描述值列表在独立证明中也可能因重复推断构造出不同运行时身份而被误拒绝。

## 选择

- 复用现有 `Custom(Calcit::Trait)` 保存具体元数据，和已有 Impl 描述值路线一致；Trait / TraitSet 仍表达实现实例的能力。不新增公开类型、检查命令、动态白名单或按函数名拦截。
- 值推断与运行时值推断都使用描述值表示。只在解析类型引用的合同上下文中形成实例能力；局部元数据选择、impl 来源和显式 trait 调用从描述值取回原声明。
- runtime Import 也沿值上下文分类；复用原 import resolver 的环守卫与查找顺序，只将最终 classifier 参数化，避免导入别名回落到 schema 分类后重新授予实例能力。schema 上下文仍保留实例合同解析，不全局更改 `from_calcit`。
- 直接声明复用 source-backed trait origin，不在每次值推断中重跑构造器。源占位与其声明可对应，两个实际求值出的 runtime ID 仍严格不同，不因名称相同合并。
- 具体描述值的运行时校验明确要求 Trait 值，不再落入未知 Custom 的宽松分支；与静态兼容共用已有名义引用关系，保留运行时身份及 qualified source placeholder 的方向性，不增加另一套身份规则。
- 描述值没有实例 impl 表是已知的空表，而不是缺失证据；复用现有缺失方法错误拒绝对描述值的实例方法调用。
- 显式 trait 调用沿原声明解析方法中的类型引用，不让导入别名把实例合同错当成描述值合同。输入检查与返回推导复用同一解析，不跳过校验。
- 移除描述值/实例混淆后，原 JS FFI 继承测试暴露出有方向证明只接受自身 TypeRef、不接受父 trait TypeRef 的缺口。复用已校验的 requires 图证明子到父，不允许反向、同名异来源或损坏 schema 借用父能力，不恢复实例到描述值的旧兼容规则。

## 验证与边界

合法语言语义放在现有 trait fixture 的 definition `:tests`：描述值列表读写、Trait 参数传递、身份相等、局部和正常 namespace 导入别名选择显式 trait 方法；原断言保留。拒绝场景扩展现有 proof harness：直接值、局部/导入别名、identity 转发、列表回调和直接方法调用都在普通、独立 proof、完整 strict、native 与 JS 编译入口检查，并验证无源码写回与无可执行产物。

私有 Rust 回归扩展已有名义匹配与 requires 证明测试，检查不能由稳定语言接口直接构造的 source placeholder / runtime ID 和损坏类型元数据关系。实际继承行为继续由原 JS FFI 附带测试与 harness 验证，不复制到新运行器。完整 Cargo、check-all、Agent 协议和已有 typed-FFI 回归仍是验收门槛；不是仅以目标 fixture 通过代替全量验证。

描述值相等的运行时规则不变，未消除已有的开放/unsafe 边界，不新增依赖或发布版本。本批不表示所有 trait 或泛型证明缺口已经解决，也不改变 0.30.1 维护线。

复审曾提出 callable schema 应持久化具体描述值身份，但尚无已验证的有效源码保存/重载回归，审查者后续明确将它降为条件性风险。正常 namespace/schema 往返保存及元数据 callback 回放通过；现有 `'Trait` 只表示元数据种类，不借此增加单例类型或 Snapshot 格式。缺少函数合同的迁移输入在补出 `'Trait` 后仍可能需要修正调用方的静态 trait 选择，这不等于原本有效的固定身份合同被破坏。
