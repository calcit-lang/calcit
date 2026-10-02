import { CalcitTag, canonicalizeTagPairs, toString } from "./calcit-data.mjs";
import { CalcitValue } from "./js-primes.mjs";
import { CalcitImpl } from "./js-impl.mjs";

export class CalcitStructDef {
  name: CalcitTag;
  fields: CalcitTag[];
  fieldTypes: CalcitValue[];
  impls: CalcitImpl[];
  cachedHash: number;
  readonly definitionRef: string | null;

  constructor(name: CalcitTag, fields: CalcitTag[], fieldTypes: CalcitValue[], impls: CalcitImpl[] = [], definitionRef: string | null = null) {
    const [canonicalFields, canonicalTypes] = canonicalizeTagPairs(fields, fieldTypes, "CalcitStructDef");
    this.name = name;
    this.fields = canonicalFields;
    this.fieldTypes = canonicalTypes;
    this.impls = impls ?? [];
    this.cachedHash = null;
    this.definitionRef = definitionRef;
  }

  withImpls(impls: CalcitImpl | CalcitImpl[]): CalcitStructDef {
    if (impls instanceof CalcitImpl) {
      return new CalcitStructDef(this.name, this.fields, this.fieldTypes, [impls], this.definitionRef);
    } else if (Array.isArray(impls)) {
      return new CalcitStructDef(this.name, this.fields, this.fieldTypes, impls, this.definitionRef);
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

/** Match native definition binding without replacing an alias's existing identity. */
export function bind_struct_definition(value: CalcitValue, definitionRef: string): CalcitValue {
  if (value instanceof CalcitStructDef && value.definitionRef == null) {
    return new CalcitStructDef(value.name, value.fields, value.fieldTypes, value.impls, definitionRef);
  }
  return value;
}
