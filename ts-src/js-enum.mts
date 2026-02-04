import { CalcitRecord } from "./js-record.mjs";

export class CalcitEnum {
  prototype: CalcitRecord;
  impls: CalcitRecord[];
  cachedHash: number;

  constructor(prototype: CalcitRecord, impls: CalcitRecord[] = []) {
    this.prototype = prototype;
    this.impls = impls;
    this.cachedHash = null;
  }

  name(): string {
    return this.prototype.name.value;
  }

  withImpls(impls: CalcitRecord | CalcitRecord[]): CalcitEnum {
    if (impls instanceof CalcitRecord) {
      return new CalcitEnum(this.prototype, [impls]);
    } else if (Array.isArray(impls)) {
      return new CalcitEnum(this.prototype, impls);
    }
    throw new Error("Expected a record as implementation");
  }

  toString(): string {
    return `(%enum :${this.prototype.name.value})`;
  }
}
