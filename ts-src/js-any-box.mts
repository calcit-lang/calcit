import type { CalcitValue } from "./js-primes.mjs";
import { option_none, option_some } from "./js-option-value.mjs";

/** Closed existential box carrying a compile-time canonical `TypeId` + typed payload. */
export class CalcitAnyBox {
  readonly typeKey: string;
  readonly value: CalcitValue;

  constructor(typeKey: string, value: CalcitValue) {
    this.typeKey = typeKey;
    this.value = value;
  }

  toString(): string {
    return "(&any-box)";
  }
}

export function any_downcast(box: CalcitValue, expectedTypeKey: string) {
  if (!(box instanceof CalcitAnyBox)) {
    throw new Error(`any:downcast expected an Any box, got: ${box}`);
  }
  return box.typeKey === expectedTypeKey ? option_some(box.value) : option_none();
}
