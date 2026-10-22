(value) => Object.assign(function counter() { return "called"; }, { "total-value": value, add(amount) { return this["total-value"] + amount; } })
