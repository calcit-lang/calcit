import { Hash } from "@calcit/ternary-tree";

import { CalcitValue } from "./js-primes.mjs";
import { _$n__$e_, newTag, toString } from "./calcit-data.mjs";
import { CalcitRecord } from "./js-record.mjs";

export class CalcitTuple {
  tag: CalcitValue;
  extra: CalcitValue[];
  klass: CalcitRecord;
  cachedHash: Hash;
  constructor(tagName: CalcitValue, extra: CalcitValue[], klass: CalcitRecord) {
    this.tag = tagName;
    this.extra = extra;
    this.klass = klass;
    this.cachedHash = null;
  }
  get(n: number) {
    if (n === 0) {
      return this.tag;
    } else if (n - 1 < this.extra.length) {
      return this.extra[n - 1];
    } else {
      throw new Error("Tuple only have 2 elements");
    }
  }
  assoc(n: number, v: CalcitValue) {
    if (n === 0) {
      return new CalcitTuple(v, this.extra, this.klass);
    } else if (n - 1 < this.extra.length) {
      let next_extra = this.extra.slice();
      next_extra[n - 1] = v;
      return new CalcitTuple(this.tag, next_extra, this.klass);
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
      content += toString(args[i], false, disableJsDataWarning);
    }
    if (this.klass instanceof CalcitRecord) {
      console.log("CLASS", this.klass);
      return `(%:: ${content} (:class ${this.klass.name.value}))`;
    } else {
      return `(:: ${content})`;
    }
  }
}
