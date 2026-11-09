# 分支类型与 cond 默认分支诊断 (#2651)

- 返回位置的 `if`/`cond`/`match` 分支都有具体类型、合并失败且有分支不满足声明时，报 `E_BRANCH_TYPE_MISMATCH`，逐个列出分支类型；有开放（Dynamic）或泛型分支时仍是 `E_FN_RETURN_UNPROVEN`。
- `cond` 最后一对不是 `true` 时，在宏展开前报 `E_COND_MISSING_DEFAULT`（带位置和写法提示），不再只有宏里 raise 的裸字符串。
- `and a b` 声明返回 Number 的旧用例（directional p30）现在得到更具体的 `E_BRANCH_TYPE_MISMATCH`。
