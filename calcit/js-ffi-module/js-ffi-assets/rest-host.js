() => Object.freeze({
  emit(...values) { return values.join(":"); },
  prefix(label, ...values) { return [label, ...values].join(":"); }
})
