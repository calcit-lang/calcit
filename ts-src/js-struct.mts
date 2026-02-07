import { CalcitTag, toString } from "./calcit-data.mjs";
import { CalcitValue } from "./js-primes.mjs";
import { CalcitRecord } from "./js-record.mjs";

export class CalcitStruct {
  name: CalcitTag;
  fields: CalcitTag[];
  fieldTypes: CalcitValue[];
  impls: CalcitRecord[];
  cachedHash: number;

  constructor(name: CalcitTag, fields: CalcitTag[], fieldTypes: CalcitValue[], impls: CalcitRecord[] = []) {
    if (fields.length !== fieldTypes.length) {
      throw new Error("CalcitStruct: fields and fieldTypes length mismatch");
    }
    this.name = name;
    this.fields = fields;
    this.fieldTypes = fieldTypes;
    this.impls = impls;
    this.cachedHash = null;
  }

  withImpls(impls: CalcitRecord | CalcitRecord[]): CalcitStruct {
    if (impls instanceof CalcitRecord) {
      return new CalcitStruct(this.name, this.fields, this.fieldTypes, [impls]);
    } else if (Array.isArray(impls)) {
      return new CalcitStruct(this.name, this.fields, this.fieldTypes, impls);
    }
    throw new Error("Expected a record as implementation");
  }

  toString(disableJsDataWarning: boolean = false): string {
    if (this.fields.length !== this.fieldTypes.length) {
      throw new Error("CalcitStruct: fields and fieldTypes length mismatch");
    }
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
