# Share relation and render budgets / 共享关系与渲染预算

## English

- Address review findings by sharing one compatibility visit budget across
  nested signatures, aliases, slots, and recursive relation calls.
- Thread the same diagnostic depth/node budget through function arguments,
  rest arguments, return types, and wide nominal argument lists.
- Add regressions for an oversized nested function relation and a deep,
  1,024-argument function diagnostic.

## 中文

- 根据审查意见，让嵌套函数签名、alias、slot 及递归关系调用共享同一个
  compatibility 访问预算。
- 函数参数、rest 参数、返回值与宽名义参数列表共同使用诊断深度/节点预算。
- 增加超宽嵌套函数关系，以及深返回值加 1,024 参数函数诊断的回归。
