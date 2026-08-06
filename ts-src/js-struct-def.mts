import { CalcitTag, toString } from "./calcit-data.mjs";
import { CalcitValue } from "./js-primes.mjs";
import { CalcitImpl } from "./js-impl.mjs";

export class CalcitStructDef {
  name: CalcitTag;
  fields: CalcitTag[];
  fieldTypes: CalcitValue[];
  impls: CalcitImpl[];
  cachedHash: number;

  constructor(name: CalcitTag, fields: CalcitTag[], fieldTypes: CalcitValue[], impls: CalcitImpl[] = []) {
    if (fields.length !== fieldTypes.length) {
      throw new Error("CalcitStructDef: fields and fieldTypes length mismatch");
    }
    this.name = name;
    this.fields = fields;
    this.fieldTypes = fieldTypes;
    this.impls = impls;
    this.cachedHash = null;
  }

  withImpls(impls: CalcitImpl | CalcitImpl[]): CalcitStructDef {
    if (impls instanceof CalcitImpl) {
      return new CalcitStructDef(this.name, this.fields, this.fieldTypes, [impls]);
    } else if (Array.isArray(impls)) {
      return new CalcitStructDef(this.name, this.fields, this.fieldTypes, impls);
    }
    throw new Error("Expected an impl as implementation");
  }

  toString(disableJsDataWarning: boolean = false): string {
    if (this.fields.length !== this.fieldTypes.length) {
      throw new Error("CalcitStructDef: fields and fieldTypes length mismatch");
    }
    const parts: string[] = ["(%struct-def '", this.name.value];
    for (let idx = 0; idx < this.fields.length; idx++) {
      const field = this.fields[idx];
      const fieldType = this.fieldTypes[idx];
      parts.push(" (:", field.value, " ", toString(fieldType, true, disableJsDataWarning), ")");
    }
    parts.push(")");
    return parts.join("");
  }
}
