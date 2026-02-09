import { Hash } from "@calcit/ternary-tree";
import { CalcitValue } from "./js-primes.mjs";
import { CalcitTag, castTag, findInFields, toString } from "./calcit-data.mjs";

export class CalcitImpl {
  name: CalcitTag;
  fields: Array<CalcitTag>;
  values: Array<CalcitValue>;
  cachedHash: Hash;

  constructor(name: CalcitTag, fields: Array<CalcitTag>, values: Array<CalcitValue>) {
    this.name = name;
    this.fields = fields;
    this.values = values;
    this.cachedHash = null;
  }

  get(k: CalcitValue) {
    let field = castTag(k);
    let idx = findInFields(this.fields, field);
    if (idx >= 0) {
      return this.values[idx];
    }
    throw new Error(`Cannot find :${field} among (${this.fields.join(",")})`);
  }

  getOrNil(k: CalcitValue) {
    let field = castTag(k);
    let idx = findInFields(this.fields, field);
    if (idx >= 0) {
      return this.values[idx];
    }
    return undefined;
  }

  toString(disableJsDataWarning: boolean = false): string {
    const parts = ["(%impl ", this.name.toString()];
    for (let idx = 0; idx < this.fields.length; idx++) {
      parts.push(" (", this.fields[idx].toString(), " ", toString(this.values[idx], true, disableJsDataWarning), ")");
    }
    parts.push(")");
    return parts.join("");
  }
}
