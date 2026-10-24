# 0.31.0 删除最后一批已弃用的核心名字

## 决策与范围

#2079 要求核心库解耦后删除 `calcit.core` 中剩余的弃用名字。本次删除 `%ok`、`%err`、`defatom`、`turn-string`、`remove-watch`、`let{}`、`let-destruct`，同时退役 `core-nominal-constructor-v1` 与 `core-ref-constructor-v1` 两条 fix 规则。删除前用 `analyze weak-types --only deprecated-call` 检查了十个核心库（Respo、Respo UI、Reel、Markdown、Recollect、lilac-parser、ws-edn、cumulo-reel、js-ffi、std）的默认分支，调用数均为 0。仍带弃用标签的只剩 `&format-ternary-tree`，按计划在 0.32.0 删除。

## 实现要点

- 编译器生成的 Result（`try-parse-cirru-edn-as`、`try-decode-map-as`、JS 的 `tmpl_result_try`）直接构造 `Result :ok/:err`，不再经过 `%ok/%err`。类型推断识别降级后的 `NativeNamedEnumNew Result :ok/:err`，if 分支的 Result 合并证据随之保留。
- `remove-watch!` 成为内建实现本身，报错文本改为 `remove-watch! failed: ...`，JS 导出改名 `remove_watch_$x_`。
- `defref` 的 syntax 不再接受 `defatom` 拼写；Snapshot、effects graph、类型覆盖率等处的定义头列表一并去掉 `defatom`。
- `turn-string` 的两个数字文本测试移到 `to-string`。内部原语 `&turn-string` 与 JS 的 `turn_string` 导出保留，`to-string` 与内建 `ToString` 实现仍依赖它们。
- 残留调用在检查时报告未知名字，告警给出删除版本和改写方式。

## 兼容边界

- `core-identity-conversion-v1` 只保留 `turn-symbol` / `turn-tag` 的 String 恒等转换，`core-api-0.28-v1` 与 `core-api-0.29-v1` 都只含这一条规则。
- 弃用报告的 CLI 测试改用项目内自定义的弃用定义，不再依赖 core 的弃用名字。
- RFC 与历史记录中的旧写法属于历史档案，不改写。

## 验证方式

- `cargo test`、`cargo clippy --all-targets -- -D warnings` 与 `yarn check-all`。
- 用 `--check-only` 检查残留调用，确认告警文本。
