# 动态 Struct 写入的可验证合同

## 依据与取舍

#1868 的真实阻塞是 Recollect 的动态 patch 写入无法通过 Diary 原 strict workflow。此前仅 `with` 具有历史例外，浅层 kind 检查会把错误的 List 成员、擦除的泛型和不同函数签名放行。前两段修复统一可解析数据合同及构造；本段让动态名称写入只在拥有运行时证据时成功，不另增公开 API 或检查命令。

继续复用 data shape 与字段校验缓存。图中的具体 Ref payload 只代表当前内容，不能证明后续修改，因此动态写入拒绝该合同；明确声明的 Ref<Dynamic> 不承诺 payload 类型。裸 Fn 与 Dynamic/JsObject 是显式开放边界，不能混同带签名 Fn 或未绑定 TypeVar。JS 的现有 type-form matcher 保留“不可验证”与“不匹配”的区别，不能以 Optional/JsNullish 包装掩盖不可验证的内部合同。

静态方法调用、普通 assoc 与已知 String key 共用已有 indexed lowering，继续保留泛型和函数/Ref 合同。真实 Diary 回放发现 core assoc 的单态化遗漏最后一步 lowering，导致合法 Respo RenderNode 更新走动态检查；在编译器修复，而不是修改业务为 native call。raw indexed source 不免除原严格证明，已知 Fn 字段接收 Dynamic 的负例仍失败。

## 验证与边界

语义回归放在原 fixture 的 Calcit definition :tests，由既有 native/JS runner 回放；覆盖静态方法/core assoc、String key、擦除泛型/带签名 Fn、直接/嵌套 Ref，以及显式开放字段。原 Struct 跨后端门禁另检查成功证明、raw indexed 负例与静态 Fn 写入借证失败，保持 source 不变。

开发分支已越过原 Diary 完整 strict 阻塞，修复 core assoc lowering 后原运行门禁也通过；提交验收仍需完整测试、最新 PR CI/review 与正式发布配对重验。未修改消费者、安装模块缓存或 strict 门禁。JS 未携带可解析合同的动态 prototype 明确拒绝，构造 fallback 不冒充转换证明；本段不扩展 WASM 动态 Struct 支持。

## 完整 CI 发现的 WASM 元数据缺口

新增的函数字段定义使同一 fixture 中原有的三项 WASM 短路测试报缺少 Struct name tag。layout 收集本来会登记整个已编译程序及运行时已解析的定义，而非 Component 边界的 tag 收集只扫描选中函数体，两者范围不一致。所有边界统一收集 layout 所需的已解析定义及嵌套引用标签，保留排序、去重和既有身份规则；不执行不支持的函数，不增加 WASM 动态写入能力。原三项 native/WASM 附带测试直接复验通过，不删除、移动或排除测试。
