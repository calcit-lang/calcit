import { CalcitRecord } from "./js-record.mjs";
import { CalcitImpl } from "./js-impl.mjs";

export class CalcitEnum {
  prototype: CalcitRecord;
  impls: CalcitImpl[];
  cachedHash: number;

  constructor(prototype: CalcitRecord, impls: CalcitImpl[] = []) {
    this.prototype = prototype;
    this.impls = impls;
    this.cachedHash = null;
  }

  name(): string {
    return this.prototype.name.value;
  }

  withImpls(impls: CalcitImpl | CalcitImpl[]): CalcitEnum {
    if (impls instanceof CalcitImpl) {
      return new CalcitEnum(this.prototype, [impls]);
    } else if (Array.isArray(impls)) {
      return new CalcitEnum(this.prototype, impls);
    }
    throw new Error("Expected an impl as implementation");
  }

  toString(): string {
    return `(%enum :${this.prototype.name.value})`;
  }
}
