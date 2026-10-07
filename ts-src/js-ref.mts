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

/** Creates a local `Ref<T>`; generated code calls this for both `ref` and the compatibility spelling `atom`. */
export let ref = (x: CalcitValue): CalcitValue => {
  atomCounter = atomCounter + 1;
  let v = new CalcitRef(x, `atom-${atomCounter}`);
  return v;
};

/** Compatibility export for JavaScript callers written before `ref`; it is the same function. */
export let atom = ref;
