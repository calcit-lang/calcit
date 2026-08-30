# 严格校验 Calx benchmark 整数参数 / Strictly validate Calx benchmark integers

## 中文

- 根据 PR review 将 benchmark 环境变量从 `Number.parseInt` 的部分解析改为完整十进制整数字符串校验。
- 拒绝尾随文本、小数、指数、空白、越过 safe-integer 范围及低于参数下限的值。
- 抽取可测试的 settings helper，并将快速 Node 回归加入 `yarn check-all`。

## English

- Replace partial `Number.parseInt` parsing with complete base-10 integer-string validation after PR review.
- Reject trailing text, fractions, exponents, whitespace, unsafe integers, and values below each setting's minimum.
- Extract a testable settings helper and add the fast Node regression to `yarn check-all`.
