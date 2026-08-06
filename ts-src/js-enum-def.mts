import { CalcitStructValue } from "./js-struct-value.mjs";
import { CalcitImpl } from "./js-impl.mjs";

export class CalcitEnumDef {
  prototype: CalcitStructValue;
  cachedHash: number;

  constructor(prototype: CalcitStructValue) {
    this.prototype = prototype;
    this.cachedHash = null;
  }

  name(): string {
    return this.prototype.name.value;
  }

  get impls(): CalcitImpl[] {
    return this.prototype.structRef.impls;
  }

  withImpls(impls: CalcitImpl | CalcitImpl[]): CalcitEnumDef {
    let nextImpls: CalcitImpl[];
    if (impls instanceof CalcitImpl) {
      nextImpls = [impls];
    } else if (Array.isArray(impls)) {
      nextImpls = impls;
    } else {
      throw new Error("Expected an impl as implementation");
    }
    return new CalcitEnumDef(this.prototype.withImpls(nextImpls));
  }

  toString(): string {
    return `(%enum-def '${this.prototype.name.value})`;
  }
}
