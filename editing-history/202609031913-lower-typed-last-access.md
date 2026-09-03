# Lower typed last access

Review of the receiver-oriented List documentation exposed that `last` was the remaining Option-returning indexed access without a built-in receiver implementation or preprocess specialization. Merely reverting the documentation would preserve an artificial gap in the typed source surface.

List, String, and Enum implementation tables now expose `.last`. Preprocessing lowers both receiver and prefix forms to family-specific primitives with explicit `%some` / `%none`; String and Enum counts are bound once before calculating the final index, and the caller receiver remains exactly-once. Map remains intentionally limited to `.get` because it has no positional last-item contract.

Definition-attached core tests cover receiver-style List, String, and empty access. Rust lowering tests verify that List `.last` selects empty/last primitives without count/nth fallback, and generated-JavaScript checks reject retained `invoke_method("last", ...)` in the typed fixture.
