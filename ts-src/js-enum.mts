import { CalcitRecord } from "./js-record.mjs";
import { CalcitImpl } from "./js-impl.mjs";

export class CalcitEnum {
  prototype: CalcitRecord;
  cachedHash: number;

  constructor(prototype: CalcitRecord) {
    this.prototype = prototype;
    this.cachedHash = null;
  }

  name(): string {
    return this.prototype.name.value;
  }

  get impls(): CalcitImpl[] {
    return this.prototype.structRef.impls;
  }

  withImpls(impls: CalcitImpl | CalcitImpl[]): CalcitEnum {
    let nextImpls: CalcitImpl[];
    if (impls instanceof CalcitImpl) {
      nextImpls = [impls];
    } else if (Array.isArray(impls)) {
      nextImpls = impls;
    } else {
      throw new Error("Expected an impl as implementation");
    }
    return new CalcitEnum(this.prototype.withImpls(nextImpls));
  }

  toString(): string {
    return `(%enum :${this.prototype.name.value})`;
  }
}
