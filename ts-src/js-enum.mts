import { CalcitRecord } from "./js-record.mjs";

export class CalcitEnum {
  prototype: CalcitRecord;
  klass: CalcitRecord;
  cachedHash: number;

  constructor(prototype: CalcitRecord, klass: CalcitRecord = null) {
    this.prototype = prototype;
    this.klass = klass;
    this.cachedHash = null;
  }

  name(): string {
    return this.prototype.name.value;
  }

  withClass(klass: CalcitRecord): CalcitEnum {
    if (klass instanceof CalcitRecord) {
      return new CalcitEnum(this.prototype, klass);
    }
    throw new Error("Expected a record as class");
  }

  toString(): string {
    return `(%enum :${this.prototype.name.value})`;
  }
}
