import { CalcitEnumDef } from "./js-enum-def.mjs";
import { CalcitEnumValue } from "./js-enum-value.mjs";
import { CalcitValue } from "./js-primes.mjs";
import { newTag } from "./calcit-data.mjs";

// Binding a reloaded definition replaces its registry entry. Primitives resolve
// at call time, rather than keeping a stale or separately constructed prototype.
const coreEnums = new Map<string, CalcitEnumDef>();

export const register_core_enum = (definitionRef: string, definition: CalcitEnumDef): void => {
  coreEnums.set(definitionRef, definition);
};

export const core_enum_value = (definitionRef: string, tag: string, payload: CalcitValue[]): CalcitEnumValue => {
  const definition = coreEnums.get(definitionRef);
  if (definition == null) {
    throw new Error(`${definitionRef} is not initialized; primitives returning nominal results need the calcit.core module to be loaded`);
  }
  return new CalcitEnumValue(newTag(tag), payload, definition);
};
