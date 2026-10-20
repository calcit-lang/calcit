import { CalcitValue } from "./js-primes.mjs";
import { CalcitEnumValue } from "./js-enum-value.mjs";
import { core_enum_value } from "./js-core-enum-value.mjs";

/** `Option :some value` */
export const option_some = (value: CalcitValue): CalcitEnumValue => core_enum_value("calcit.core/Option", "some", [value]);

/** `Option :none` */
export const option_none = (): CalcitEnumValue => core_enum_value("calcit.core/Option", "none", []);
