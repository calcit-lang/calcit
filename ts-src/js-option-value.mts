import { CalcitValue } from "./js-primes.mjs";
import { CalcitEnumDef } from "./js-enum-def.mjs";
import { CalcitEnumValue } from "./js-enum-value.mjs";
import { newTag } from "./calcit-data.mjs";

// Primitives that report absence return the nominal `calcit.core/Option`
// (#1554). The enum definition lives in compiled `calcit.core`; it registers
// itself here when the module binds its `Option` definition, so values built by
// primitives share the definition (and its method impls) with `Option :some x`.
let coreOption: CalcitEnumDef | null = null;

export const register_core_option = (definition: CalcitEnumDef): void => {
  coreOption = definition;
};

const requireCoreOption = (): CalcitEnumDef => {
  if (coreOption == null) {
    throw new Error("calcit.core/Option is not initialized; primitives returning Option need the calcit.core module to be loaded");
  }
  return coreOption;
};

/** `Option :some value` */
export const option_some = (value: CalcitValue): CalcitEnumValue => new CalcitEnumValue(newTag("some"), [value], requireCoreOption());

/** `Option :none` */
export const option_none = (): CalcitEnumValue => new CalcitEnumValue(newTag("none"), [], requireCoreOption());
