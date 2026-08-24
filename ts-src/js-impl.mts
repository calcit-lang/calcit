import { Hash } from "@calcit/ternary-tree";
import { CalcitValue } from "./js-primes.mjs";
import { CalcitTag, canonicalizeTagPairs, castTag, findInFields, toString } from "./calcit-data.mjs";
import type { CalcitTrait } from "./js-trait.mjs";

const CALCIT_IMPL_BRAND = Symbol.for("@calcit/procs/CalcitImpl");

export class CalcitImpl {
  name: CalcitTag;
  origin: CalcitTrait | null;
  fields: Array<CalcitTag>;
  values: Array<CalcitValue>;
  cachedHash: Hash;

  static [Symbol.hasInstance](value: unknown): boolean {
    if (typeof value !== "object" || value === null) return false;

    const candidate = value as Record<PropertyKey, unknown>;
    const brand = Object.getOwnPropertyDescriptor(candidate, CALCIT_IMPL_BRAND);
    const name = Object.getOwnPropertyDescriptor(candidate, "name");
    const origin = Object.getOwnPropertyDescriptor(candidate, "origin");
    const fields = Object.getOwnPropertyDescriptor(candidate, "fields");
    const values = Object.getOwnPropertyDescriptor(candidate, "values");
    const cachedHash = Object.getOwnPropertyDescriptor(candidate, "cachedHash");

    return (
      brand?.value === true &&
      name !== undefined &&
      origin !== undefined &&
      fields !== undefined &&
      values !== undefined &&
      cachedHash !== undefined &&
      typeof name.value === "object" &&
      name.value !== null &&
      typeof (name.value as { value?: unknown }).value === "string" &&
      (origin.value === null || typeof origin.value === "object") &&
      Array.isArray(fields.value) &&
      Array.isArray(values.value)
    );
  }

  constructor(name: CalcitTag, fields: Array<CalcitTag>, values: Array<CalcitValue>, origin: CalcitTrait | null = null) {
    // Vite can temporarily retain two copies of @calcit/procs while refreshing
    // optimized dependencies. A global, non-enumerable brand keeps impl values
    // recognizable across those otherwise distinct module instances.
    Object.defineProperty(this, CALCIT_IMPL_BRAND, { value: true });
    const [canonicalFields, canonicalValues] = canonicalizeTagPairs(fields, values, "CalcitImpl");
    this.name = name;
    this.origin = origin;
    this.fields = canonicalFields;
    this.values = canonicalValues;
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
