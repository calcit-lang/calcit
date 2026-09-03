# Make data-to-code map output deterministic

- Sort Map entries by Calcit's canonical value order before converting them into generated code.
- Apply the order recursively through nested Map values, so identical input data produces stable generated source across processes.
- Cover nested Map output with a direct conversion regression test.
