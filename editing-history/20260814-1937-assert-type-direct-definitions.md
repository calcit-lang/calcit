# 2026-08-14 assert-type 直接类型定义

- `assert-type` 的第二参数是类型语境；未加引号的 symbol 现在会解析为可见的 Struct/Enum 定义，例如 `assert-type source Store`。
- 单引号继续用于 TypeVar（`'T`）和显式名义 TypeRef，不把泛型变量当作运行时定义求值。
- 两条 Struct 收窄回归测试改为直接定义写法，覆盖语句断言及 `let` 右侧断言。
