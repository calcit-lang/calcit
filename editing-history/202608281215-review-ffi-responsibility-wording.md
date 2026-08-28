# 修正文档责任边界措辞 / Clarify responsibility-boundary wording

## 中文

- 根据 PR review 将模块职责改为非穷举表述。
- 明确业务参数与逻辑、thread、connection/task registry、cancel state、server lifecycle 和其他领域状态仍由模块维护。

## English

- Make the module-responsibility wording non-exhaustive in response to PR review.
- Clarify that modules continue to own domain arguments and behavior, threads, connection/task registries, cancellation state, server lifecycles, and other domain-specific state.
