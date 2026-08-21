# Quality metric name comparison

The static-quality gate compares each metric to its limit by its stable wire name,
rather than relying on the parallel ordering of two metric arrays. This keeps new
or reordered metrics from silently being compared with a different budget.
