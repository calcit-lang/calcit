import { CalcitTag, canonicalizeTagPairs, toString } from "./calcit-data.mjs";
import { CalcitValue } from "./js-primes.mjs";
import { CalcitImpl } from "./js-impl.mjs";
import { CalcitEnumDef } from "./js-enum-def.mjs";
import { CalcitStructValue } from "./js-struct-value.mjs";

export type FieldValidator = ((value: CalcitValue, requireEvidence?: boolean) => boolean) | null;

export class CalcitStructDef {
  name: CalcitTag;
  fields: CalcitTag[];
  fieldTypes: CalcitValue[];
  impls: CalcitImpl[];
  cachedHash: number;
  readonly definitionRef: string | null;
  readonly fieldValidators: FieldValidator[] | null;

  constructor(name: CalcitTag, fields: CalcitTag[], fieldTypes: CalcitValue[], impls: CalcitImpl[] = [], definitionRef: string | null = null,
    fieldValidators: FieldValidator[] | null = null) {
    const [canonicalFields, canonicalTypes] = canonicalizeTagPairs(fields, fieldTypes, "CalcitStructDef");
    this.name = name;
    this.fields = canonicalFields;
    this.fieldTypes = canonicalTypes;
    this.impls = impls ?? [];
    this.cachedHash = null;
    this.definitionRef = definitionRef;
    this.fieldValidators = fieldValidators == null ? null : canonicalizeTagPairs(fields, fieldValidators, "CalcitStructDef validators")[1];
  }

  withImpls(impls: CalcitImpl | CalcitImpl[]): CalcitStructDef {
    if (impls instanceof CalcitImpl) {
      return new CalcitStructDef(this.name, this.fields, this.fieldTypes, [impls], this.definitionRef, this.fieldValidators);
    } else if (Array.isArray(impls)) {
      return new CalcitStructDef(this.name, this.fields, this.fieldTypes, impls, this.definitionRef, this.fieldValidators);
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

/** Bind nominal definitions like native, preserving an alias's existing identity. */
export function bind_struct_definition(value: CalcitValue, definitionRef: string, fieldValidators: FieldValidator[] | null = null): CalcitValue {
  if (value instanceof CalcitStructDef && value.definitionRef == null) {
    return new CalcitStructDef(value.name, value.fields, value.fieldTypes, value.impls, definitionRef, fieldValidators);
  }
  if (value instanceof CalcitEnumDef && value.prototype.structRef.definitionRef == null) {
    const prototype = value.prototype;
    const structRef = bind_struct_definition(prototype.structRef, definitionRef) as CalcitStructDef;
    return new CalcitEnumDef(new CalcitStructValue(prototype.name, prototype.fields, prototype.values, structRef));
  }
  return value;
}
