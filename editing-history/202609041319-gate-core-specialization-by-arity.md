# Gate core specialization by arity / 按 arity 筛选 core 专门化

Copilot follow-up review identified that eligible core function names still resolved their receiver before proving that enough actual and expected arguments existed. The specializer now derives the required arity from the callee name and exits before receiver inference or expected-type cloning when an invalid-arity call cannot be specialized.

Copilot 后续 review 指出，即使 core 函数名符合条件，specializer 仍会在确认参数数量足够前解析 receiver。现根据 callee 名称先确定所需 arity；无效 arity 调用无法专门化时，会在 receiver 推断和 expected type clone 之前退出。
