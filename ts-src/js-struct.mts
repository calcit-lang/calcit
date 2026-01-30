import { CalcitTag, toString } from "./calcit-data.mjs";
import { CalcitValue } from "./js-primes.mjs";
import { CalcitRecord } from "./js-record.mjs";

export class CalcitStruct {
  name: CalcitTag;
  fields: CalcitTag[];
  fieldTypes: CalcitValue[];
  klass: CalcitRecord;
  cachedHash: number;

  constructor(name: CalcitTag, fields: CalcitTag[], fieldTypes: CalcitValue[], klass: CalcitRecord = null) {
    this.name = name;
    this.fields = fields;
    this.fieldTypes = fieldTypes;
    this.klass = klass;
    this.cachedHash = null;
  }

  withClass(klass: CalcitRecord): CalcitStruct {
    if (klass instanceof CalcitRecord) {
      return new CalcitStruct(this.name, this.fields, this.fieldTypes, klass);
    }
    throw new Error("Expected a record as class");
  }

  toString(disableJsDataWarning: boolean = false): string {
    const parts: string[] = ["(%struct :", this.name.value];
    for (let idx = 0; idx < this.fields.length; idx++) {
      const field = this.fields[idx];
      const fieldType = this.fieldTypes[idx];
      parts.push(" (:", field.value, " ", toString(fieldType, true, disableJsDataWarning), ")");
    }
    parts.push(")");
    return parts.join("");
  }
}
