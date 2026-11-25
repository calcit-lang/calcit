import { CalcitValue } from "./js-primes.mjs";
import { CalcitTrait } from "./js-trait.mjs";

export class CalcitDynTrait {
  traitDef: CalcitTrait;
  value: CalcitValue;
  vtable: Map<string, (...args: CalcitValue[]) => CalcitValue>;

  constructor(
    traitDef: CalcitTrait,
    value: CalcitValue,
    vtable: Map<string, (...args: CalcitValue[]) => CalcitValue>,
  ) {
    this.traitDef = traitDef;
    this.value = value;
    this.vtable = vtable;
  }

  toString(): string {
    return `(&dyn ${this.traitDef.name.value})`;
  }
}
