# Option / Result 比较的名义证据

Timegrass 升级到正式 0.29.0 时，原持久化测试比较预期空 Option 与名义记录字段。补全泛型类型后，普通相等合同已匹配，但 legacy 校验仍报告 nullable 用法。独立 `OptionBox` 复现不依赖业务模块，同一问题在 main 候选上存在。

原因是已有识别器只识别全限定 `TypeRef("calcit.core/Option", ...)` / Result；local hint 可保留短 TypeRef，字段读取则携带完整引用。识别器现在对 TypeRef 复用现有 enum resolver 与完整 definition ref；不按短名猜测，不把用户自己定义的同名 enum 当作 core Option，也不改相等的泛型关系。不扩大直接 Enum / EnumValue 的 legacy 检测范围，现有通用 nil 谓词回放保持原契约。

用户语义回归放在 `same-nominal-absence-fields` 的 definition `:tests`，覆盖 Number/String Option、Result 的相同和不同 payload/variant，并接入既有 native/JS/WASM 运行器。原 Option 与 nil/payload 的 legacy 检查及相等类型负例继续执行。没有新诊断、fixed fix rule、unsafe 或 Dynamic 放宽。

Timegrass 本地正式图作为真实消费者复验；语言源码修复不等于它已完成正式新版工具链升级或发布。其客户端状态树仍有独立的开放边界问题，须分别完成原严格与业务门禁。
