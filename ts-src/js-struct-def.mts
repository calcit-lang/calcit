import { CalcitSymbol, CalcitTag, canonicalizeTagPairs, toString, _$n__$e_ } from "./calcit-data.mjs";
import { CalcitValue } from "./js-primes.mjs";
import { CalcitImpl } from "./js-impl.mjs";
import { CalcitEnumDef } from "./js-enum-def.mjs";
import { CalcitStructValue, fieldsEqual } from "./js-struct-value.mjs";
import { register_core_enum } from "./js-core-enum-value.mjs";
import { typeFormIdentity } from "./js-type-form.mjs";

export type FieldValidator = ((value: CalcitValue, requireEvidence?: boolean) => boolean) | null;

export class CalcitStructDef {
  name: CalcitTag;
  fields: CalcitTag[];
  fieldTypes: CalcitValue[];
  readonly fieldTypeIdentities: CalcitValue[];
  impls: CalcitImpl[];
  cachedHash: number;
  readonly definitionRef: string | null;
  readonly fieldValidators: FieldValidator[] | null;
  readonly generics: CalcitValue[];
  readonly whereBounds: CalcitValue;
  /** Declared literal defaults aligned with `fields`, empty when no field has one. Preprocess fills them in. */
  readonly fieldDefaults: CalcitValue[];

  constructor(name: CalcitTag, fields: CalcitTag[], fieldTypes: CalcitValue[], impls: CalcitImpl[] = [], definitionRef: string | null = null,
    fieldValidators: FieldValidator[] | null = null, generics: CalcitValue[] = [], whereBounds: CalcitValue = null, fieldDefaults: CalcitValue[] = []) {
    const [canonicalFields, canonicalTypes] = canonicalizeTagPairs(fields, fieldTypes, "CalcitStructDef");
    this.name = name;
    this.fields = canonicalFields;
    this.fieldTypes = canonicalTypes;
    const genericNames = generics.map(item => item instanceof CalcitSymbol ? item.value : "");
    this.fieldTypeIdentities = canonicalTypes.map(form => typeFormIdentity(form, genericNames));
    this.impls = impls ?? [];
    this.cachedHash = null;
    this.definitionRef = definitionRef;
    this.fieldValidators = fieldValidators == null ? null : canonicalizeTagPairs(fields, fieldValidators, "CalcitStructDef validators")[1];
    this.generics = generics;
    this.whereBounds = whereBounds;
    this.fieldDefaults = fieldDefaults.length === 0 ? [] : canonicalizeTagPairs(fields, fieldDefaults, "CalcitStructDef defaults")[1];
  }

  withImpls(impls: CalcitImpl | CalcitImpl[]): CalcitStructDef {
    if (impls instanceof CalcitImpl) {
      return new CalcitStructDef(this.name, this.fields, this.fieldTypes, [impls], this.definitionRef, this.fieldValidators, this.generics, this.whereBounds, this.fieldDefaults);
    } else if (Array.isArray(impls)) {
      return new CalcitStructDef(this.name, this.fields, this.fieldTypes, impls, this.definitionRef, this.fieldValidators, this.generics, this.whereBounds, this.fieldDefaults);
    }
    throw new Error("Expected an impl as implementation");
  }

  /** Nominal identity for value matching; declared defaults only affect construction, so they are excluded. */
  sameNominalDefinition(other: CalcitStructDef): boolean {
    return this.name === other.name && this.definitionRef === other.definitionRef && fieldsEqual(this.fields, other.fields)
      && this.fieldTypeIdentities.length === other.fieldTypeIdentities.length
      && this.fieldTypeIdentities.every((type, index) => _$n__$e_(type, other.fieldTypeIdentities[index]))
      && this.generics.length === other.generics.length && this.generics.every((generic, index) => _$n__$e_(generic, other.generics[index]))
      && _$n__$e_(this.whereBounds, other.whereBounds);
  }

  /** Data-schema identity excludes attached method implementations. */
  sameDataSchema(other: CalcitStructDef): boolean {
    return this.sameNominalDefinition(other)
      && this.fieldDefaults.length === other.fieldDefaults.length
      && this.fieldDefaults.every((value, index) => _$n__$e_(value, other.fieldDefaults[index]));
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
    return new CalcitStructDef(value.name, value.fields, value.fieldTypes, value.impls, definitionRef, fieldValidators, value.generics, value.whereBounds, value.fieldDefaults);
  }
  if (value instanceof CalcitEnumDef && value.prototype.structRef.definitionRef == null) {
    const prototype = value.prototype;
    const structRef = bind_struct_definition(prototype.structRef, definitionRef) as CalcitStructDef;
    const bound = new CalcitEnumDef(new CalcitStructValue(prototype.name, prototype.fields, prototype.values, structRef));
    if (definitionRef.startsWith("calcit.core/")) register_core_enum(definitionRef, bound);
    return bound;
  }
  return value;
}
