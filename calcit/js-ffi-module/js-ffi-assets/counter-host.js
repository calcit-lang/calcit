(value) => Object.freeze({ "total-value": value, add(amount) { return this["total-value"] + amount; } })
