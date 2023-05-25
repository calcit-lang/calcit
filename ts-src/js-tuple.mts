import { Hash } from "@calcit/ternary-tree";

import { CalcitValue } from "./js-primes.mjs";
import { toString } from "./calcit-data.mjs";

export class CalcitTuple {
  tag: CalcitValue;
  extra: CalcitValue[];
  cachedHash: Hash;
  constructor(tag: CalcitValue, extra: CalcitValue[]) {
    this.tag = tag;
    this.extra = extra;
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
      return new CalcitTuple(v, this.extra);
    } else if (n - 1 < this.extra.length) {
      let next_extra = this.extra.slice();
      next_extra[n - 1] = v;
      return new CalcitTuple(this.tag, next_extra);
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
