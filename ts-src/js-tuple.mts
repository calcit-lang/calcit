import { Hash } from "@calcit/ternary-tree";

import { CalcitValue } from "./js-primes.mjs";
import { newTag, toString } from "./calcit-data.mjs";
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
  toString(disableJsDataWarning: boolean = false): string {
    let args = [this.tag, ...this.extra];
    let content = "";
    for (let i = 0; i < args.length; i++) {
      if (i > 0) {
        content += " ";
      }
      content += toString(args[i], false, disableJsDataWarning);
    }
    return `(&tuple ${content})`;
  }
}
