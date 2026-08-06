import { Hash } from "@calcit/ternary-tree";

import { CalcitValue } from "./js-primes.mjs";
import { _$n__$e_, newTag, toString } from "./calcit-data.mjs";
import { CalcitImpl } from "./js-impl.mjs";
import { CalcitEnumDef } from "./js-enum-def.mjs";

export class CalcitEnumValue {
  tag: CalcitValue;
  extra: CalcitValue[];
  enumPrototype: CalcitEnumDef;
  cachedHash: Hash;
  constructor(tagName: CalcitValue, extra: CalcitValue[], enumPrototype: CalcitEnumDef = null) {
    this.tag = tagName;
    this.extra = extra;
    this.enumPrototype = enumPrototype;
    this.cachedHash = null;
  }

  get impls(): CalcitImpl[] {
    if (this.enumPrototype == null) {
      return [];
    }
    return this.enumPrototype.impls;
  }

  get(n: number) {
    if (n === 0) {
      return this.tag;
    } else if (n - 1 < this.extra.length) {
      return this.extra[n - 1];
    } else {
      throw new Error(`Enum value only has ${this.extra.length + 1} elements`);
    }
  }
  assoc(n: number, v: CalcitValue) {
    if (n === 0) {
      return new CalcitEnumValue(v, this.extra, this.enumPrototype);
    } else if (n - 1 < this.extra.length) {
      let next_extra = this.extra.slice();
      next_extra[n - 1] = v;
      return new CalcitEnumValue(this.tag, next_extra, this.enumPrototype);
    } else {
      throw new Error(`Enum value only has ${this.extra.length} elements`);
    }
  }
  count() {
    return 1 + this.extra.length;
  }
  eq(y: CalcitEnumValue): boolean {
    if (!_$n__$e_(this.tag, y.tag)) {
      return false;
    }
    if (this.extra.length !== y.extra.length) {
      return false;
    }
    for (let idx = 0; idx < this.extra.length; idx++) {
      if (!_$n__$e_(this.extra[idx], y.extra[idx])) {
        return false;
      }
    }
    return true;
  }
  toString(disableJsDataWarning: boolean = false): string {
    let args = [this.tag, ...this.extra];
    let content = "";
    for (let i = 0; i < args.length; i++) {
      if (i > 0) {
        content += " ";
      }
      content += toString(args[i], true, disableJsDataWarning);
    }
    const hasEnum = this.enumPrototype != null;
    const enumName = hasEnum ? this.enumPrototype.name() : null;

    return `(%:: ${hasEnum ? `'${enumName}` : "_"} ${content})`;
  }
}
