import { CrDataValue } from "./js-primes";

import { Hash } from "@calcit/ternary-tree";

export class CrDataTuple {
  fst: CrDataValue;
  snd: CrDataValue;
  cachedHash: Hash;
  constructor(a: CrDataValue, b: CrDataValue) {
    this.fst = a;
    this.snd = b;
  }
  get(n: number) {
    if (n == 0) {
      return this.fst;
    } else if (n == 1) {
      return this.snd;
    } else {
      throw new Error("Tuple only have 2 elements");
    }
  }
  assoc(n: number, v: CrDataValue) {
    if (n == 0) {
      return new CrDataTuple(v, this.snd);
    } else if (n == 1) {
      return new CrDataTuple(this.fst, v);
    } else {
      throw new Error("Tuple only have 2 elements");
    }
  }
  toString(): string {
    return `(&tuple ${this.fst.toString()} ${this.snd.toString()})`;
  }
}
