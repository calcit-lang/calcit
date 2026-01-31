# 变更计划: Record Struct 与 Enum 语法

## 目标

- 为 record 提供“结构体”定义语法，描述字段及类型。
- 为 enum 提供独立语法，替换 record 作为 enum prototype 的临时用法。
- 使用 %:: 语法构造 enum tuple；class 统一通过 with-class 绑定。
- 约定 nil 标注自动等价于 :dynamic；不提供简写省略类型。

## 任务拆分

### 1. 语法设计与解析

- [x] 确定语法关键词：`defstruct` / `defenum`（或其它命名）
- [x] 在 Cirru 语法解析与宏处理层新增语法入口
- [x] 维护兼容：保留旧的 enum record 解析路径（可加 deprecate 警告）
- [x] 使用 %:: enum tuple 语法，标记 %%:: 为 deprecated，提供迁移提示
- [-] 不计划：强制类型标注不允许简写（省略类型）

### 2. 类型系统/注解

- [x] 增加 struct/enum 的类型描述对象（或复用现有结构）
- [x] 在 `assert-type` / `hint-fn` 等路径支持引用新结构
- [x] 支持字段类型为内置 tag 或 `:dynamic`

### 3. 运行时与内建函数

- [x] enum 新语法产出的原型应能直接供 tuple 构造使用
- [x] 保留 `new_enum_tuple` 等现有 API 行为
- [x] 评估是否需要新增 `&enum:*` 访问函数（已新增 `&enum:with-class`/`&struct:with-class`）
- [x] 统一 class 绑定写法：record/tuple 使用 `&record:with-class` / `&tuple:with-class`

### 4. 代码生成与 IR

- [x] 更新 codegen/IR 输出包含 struct/enum 信息（如有）
- [x] 保证 JS backend 与 Rust backend 行为一致

### 5. 迁移与测试

- [x] 为新语法添加测试用例（enum/struct 基本功能）
- [x] 逐步替换测试中的 record-as-enum
- [x] 替换示例中 %%:: 为 %::，并将 class 绑定迁移到 with-class
- [x] 保持 `yarn check-all` / `cargo test` 通过（已通过 yarn check-all 与 cargo test）

## 核心 struct 变更计划

### 1) 结构体定义模型

- [x] 新增 `CalcitStruct` 作为结构定义承载类型与字段信息
- [x] `CalcitRecord` 仅保留运行时值数据，并引用 `CalcitStruct` 以简化结构元信息
- [x] 明确结构信息与运行时值分离（struct 元信息 vs record 值）

### 2) CalcitTypeAnnotation 接入

- [x] 允许引用 defstruct/defenum 的名称作为类型标注
- [x] nil 标注自动映射为 :dynamic

### 3) 现有核心结构适配

- [x] 调整 `CalcitRecord` 的字段布局以引用 `CalcitStruct`
- [x] 保持 `CalcitTuple` 不引用 `CalcitStruct`；仅与 `CalcitEnum` 关联
- [x] 为 `CalcitEnum` 增加“新语法定义”与“运行时 enum 值”两种变种（record/tuple 依然保留）
- [x] 保持 `CalcitEnum` 的 lookup 索引不变，避免性能回退

## 风险点

- 旧语法与新语法同时存在时的歧义
- defstruct 会生成运行时 struct；实际数据仍是 record，struct 只承担类型/结构功能但可在运行时出现

## 里程碑

1. 语法 & AST 支持
2. 类型注解支持
3. 测试迁移与兼容期验证
