# Totally order numeric keys during code emission

Follow-up review identified that runtime `Calcit::Ord` treats unordered numeric comparisons as equal. A stable sort would therefore preserve the randomized Map iteration order when different NaN keys compare equal, leaving `&data-to-code` nondeterministic for those inputs.

Map code emission now applies `f64::total_cmp` when both keys are numbers and retains the existing Calcit ordering for every other key family. This is deliberately local to serialization: runtime Map equality and ordering semantics do not change. Regression coverage uses distinct NaN payloads with attached values to verify their emitted order.
