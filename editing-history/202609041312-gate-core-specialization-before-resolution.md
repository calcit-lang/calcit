# Gate core specialization before receiver resolution / 在 receiver 解析前筛选 core 专门化

Copilot review identified that the generalized call-site helper resolved receiver types for every `calcit.core/*` call even though only `get`, `includes?`, and `update` can specialize. Since receiver resolution may recurse into expression inference, the function-name gate now runs first and keeps unrelated core calls off this preprocessing path.

Copilot review 指出，泛化后的调用点 helper 会为所有 `calcit.core/*` 调用解析 receiver，而实际只有 `get`、`includes?`、`update` 能专门化。receiver 解析可能继续进入表达式推断，因此现将函数名筛选前移，避免无关 core 调用承担额外预处理开销。
