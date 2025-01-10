import { Hash } from "@calcit/ternary-tree";
import type { CalcitFn } from "./calcit-data.mjs";
import type { CalcitValue } from "./js-primes.mts";

export class CalcitRef {
  value: CalcitValue;
  path: string;
  listeners: Map<CalcitValue, CalcitFn>;
  cachedHash: Hash;
  constructor(x: CalcitValue, path: string) {
    this.value = x;
    this.path = path;
    this.listeners = new Map();
    this.cachedHash = null;
  }
  toString(): string {
    return `(&ref ${this.value.toString()})`;
  }
}

var atomCounter = 0;

export let atom = (x: CalcitValue): CalcitValue => {
  atomCounter = atomCounter + 1;
  let v = new CalcitRef(x, `atom-${atomCounter}`);
  return v;
};
