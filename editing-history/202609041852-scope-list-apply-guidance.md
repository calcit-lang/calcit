# Scope List.apply guidance precisely

The initial migration guide described generic substitution as a universal behavior for later arguments of every user function. This PR deliberately implements targeted `List.apply` specialization because a broad eager-substitution experiment interfered with the established `Option` nil-lifting rules.

The guide now states the exact supported behavior: direct and method-form `List.apply` diagnostics use the receiver/input-bound `T` to show the concrete callback type. No code or runtime contract changed in this follow-up.
