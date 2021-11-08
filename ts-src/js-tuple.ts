import { CalcitValue } from "./js-primes";

import { Hash } from "@calcit/ternary-tree";

export class CalcitTuple {
  fst: CalcitValue;
  snd: CalcitValue;
  cachedHash: Hash;
  constructor(a: CalcitValue, b: CalcitValue) {
    this.fst = a;
    this.snd = b;
    this.cachedHash = null;
  }
  get(n: number) {
    if (n === 0) {
      return this.fst;
    } else if (n === 1) {
      return this.snd;
    } else {
      throw new Error("Tuple only have 2 elements");
    }
  }
  assoc(n: number, v: CalcitValue) {
    if (n === 0) {
      return new CalcitTuple(v, this.snd);
    } else if (n === 1) {
      return new CalcitTuple(this.fst, v);
    } else {
      throw new Error("Tuple only have 2 elements");
    }
  }
  toString(): string {
    return `(&tuple ${this.fst.toString()} ${this.snd.toString()})`;
  }
}
