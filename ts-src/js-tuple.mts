import { Hash } from "@calcit/ternary-tree";

import { CalcitValue } from "./js-primes.mjs";
import { toString } from "./calcit-data.mjs";

export class CalcitTuple {
  fst: CalcitValue;
  snd: CalcitValue;
  extra: CalcitValue[];
  cachedHash: Hash;
  constructor(a: CalcitValue, b: CalcitValue, extra: CalcitValue[]) {
    this.fst = a;
    this.snd = b;
    this.extra = extra;
    this.cachedHash = null;
  }
  get(n: number) {
    if (n === 0) {
      return this.fst;
    } else if (n === 1) {
      return this.snd;
    } else if (n - 2 < this.extra.length) {
      return this.extra[n - 2];
    } else {
      throw new Error("Tuple only have 2 elements");
    }
  }
  assoc(n: number, v: CalcitValue) {
    if (n === 0) {
      return new CalcitTuple(v, this.snd, this.extra);
    } else if (n === 1) {
      return new CalcitTuple(this.fst, v, this.extra);
    } else if (n - 2 < this.extra.length) {
      let next_extra = this.extra.slice();
      next_extra[n - 2] = v;
      return new CalcitTuple(this.fst, this.snd, next_extra);
    } else {
      throw new Error("Tuple only have 2 elements");
    }
  }
  count() {
    return 2 + this.extra.length;
  }
  toString(disableJsDataWarning: boolean = false): string {
    return `(&tuple ${toString(this.fst, false, disableJsDataWarning)} ${toString(this.snd, false, disableJsDataWarning)})`;
  }
}
