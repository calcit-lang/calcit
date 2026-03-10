import { Hash } from "@calcit/ternary-tree";

import { CalcitValue } from "./js-primes.mjs";
import { _$n__$e_, newTag, toString } from "./calcit-data.mjs";
import { CalcitImpl } from "./js-impl.mjs";
import { CalcitRecord } from "./js-record.mjs";
import { CalcitEnum } from "./js-enum.mjs";

export class CalcitTuple {
  tag: CalcitValue;
  extra: CalcitValue[];
  enumPrototype: CalcitRecord | CalcitEnum;
  cachedHash: Hash;
  constructor(tagName: CalcitValue, extra: CalcitValue[], enumPrototype: CalcitRecord | CalcitEnum = null) {
    this.tag = tagName;
    this.extra = extra;
    this.enumPrototype = enumPrototype;
    this.cachedHash = null;
  }

  get impls(): CalcitImpl[] {
    if (this.enumPrototype == null) {
      return [];
    }
    if (this.enumPrototype instanceof CalcitEnum) {
      return this.enumPrototype.impls;
    }
    return this.enumPrototype.structRef.impls;
  }

  get(n: number) {
    if (n === 0) {
      return this.tag;
    } else if (n - 1 < this.extra.length) {
      return this.extra[n - 1];
    } else {
      throw new Error(`Tuple only have ${this.extra.length + 1} elements`);
    }
  }
  assoc(n: number, v: CalcitValue) {
    if (n === 0) {
      return new CalcitTuple(v, this.extra, this.enumPrototype);
    } else if (n - 1 < this.extra.length) {
      let next_extra = this.extra.slice();
      next_extra[n - 1] = v;
      return new CalcitTuple(this.tag, next_extra, this.enumPrototype);
    } else {
      throw new Error(`Tuple only have ${this.extra.length} elements`);
    }
  }
  count() {
    return 1 + this.extra.length;
  }
  eq(y: CalcitTuple): boolean {
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
    const enumName = hasEnum ? (this.enumPrototype instanceof CalcitEnum ? this.enumPrototype.prototype.name.value : this.enumPrototype.name.value) : null;

    if (hasEnum) {
      return `(%:: ${content} (:enum ${enumName}))`;
    }
    return `(:: ${content})`;
  }
}
