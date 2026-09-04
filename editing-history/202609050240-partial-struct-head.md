# Detect partial Struct constructors before expansion

2026-09-05 02:40 CST

## English

- Identify both partial Struct constructor spellings from the resolved call head before macro expansion.
- Do not use the enclosing definition name as call identity.
- Document both spellings and their complete-Struct or Option-field migration path.

## 中文

- 在宏展开前，从已解析调用头识别两种 partial Struct 构造写法。
- 不再误用外围 definition name 作为被调用对象身份。
- 文档同时列出两种写法，以及迁移到完整 Struct 或 Option 字段的路径。
