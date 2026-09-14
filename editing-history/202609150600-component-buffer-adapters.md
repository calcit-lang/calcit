# 增加 Component Buffer adapter

- 让类型推导得到的 `Buffer` schema 直接进入同步 Component boundary，不增加新的类型声明、检测规则或命令入口。
- Canonical ABI 严格映射为 WIT `list<u8>`；core 参数使用 `(ptr,len)`，结果使用 return area，并以独立 Buffer tag 与 UTF-8 String 保持类型隔离。
- 通过 Calcit definition `:tests` 覆盖 Buffer 与 Bool/Buffer 分支语义；通过 Rust/Node 集成测试覆盖空值、内嵌零、非 UTF-8 字节、host import/export 及越界内存 trap。
- 更新 Component boundary、CLI 与 Agent 文档，保留 0.14.20 Number/String preview 的历史范围，并说明 0.15.1 开发线新增 Bool 与 Buffer。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`、文档检查与打包清单。
