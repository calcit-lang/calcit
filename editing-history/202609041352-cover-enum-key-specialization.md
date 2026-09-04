# Cover Enum key specialization / 覆盖 Enum 键专门化

Copilot review identified that the initial `contains?` and `assoc` extension omitted concrete Enum receivers even though both operations require a numeric payload index at runtime. The specializer now treats anonymous, value, and resolvable nominal Enum annotations like the existing typed `get` path: `contains?` and `assoc` both require `Number` for the index.

The `assoc` replacement value remains deliberately open when there is no precise variant/slot evidence because Enum payload positions may be heterogeneous. Unit tests and the real Snapshot fixture now cover the Enum index mismatch, bringing the fixture to fourteen focused warnings.

Copilot review 指出首版 `contains?` 与 `assoc` 扩展遗漏了 concrete Enum receiver，而两项运行时操作都要求数值 payload index。specializer 现在与既有 typed `get` 路径一致，对 anonymous、value 及可解析 nominal Enum annotation 强制 `Number` index。

在缺少精确 variant/slot evidence 时，`assoc` replacement value 仍保持开放，因为 Enum payload 位置可以异构。单元测试与真实 Snapshot fixture 均增加 Enum index 错配覆盖，fixture 总计十四条聚焦告警。
