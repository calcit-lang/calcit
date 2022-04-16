import { CalcitKeyword, CalcitSymbol, CalcitRef, CalcitFn, CalcitRecur } from "./calcit-data.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";
import { CalcitRecord } from "./js-record.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitSet as CalcitSet } from "./js-set.mjs";
import { CalcitTuple } from "./js-tuple.mjs";

export type CalcitValue =
  | string
  | number
  | boolean
  | CalcitMap
  | CalcitSliceMap
  | CalcitList
  | CalcitSliceList
  | CalcitSet
  | CalcitKeyword
  | CalcitSymbol
  | CalcitRef
  | CalcitTuple
  | CalcitFn
  | CalcitRecur // should not be exposed to function
  | CalcitRecord
  | null;

export let is_literal = (x: CalcitValue): boolean => {
  if (x == null) return true;
  if (typeof x == "string") return true;
  if (typeof x == "boolean") return true;
  if (typeof x == "number") return true;
  if (x instanceof CalcitKeyword) return true;
  if (x instanceof CalcitSymbol) return true;
  return false;
};

enum PseudoTypeIndex {
  nil,
  bool,
  number,
  symbol,
  keyword,
  string,
  ref,
  tuple,
  recur,
  list,
  set,
  map,
  record,
  fn,
}

let typeAsInt = (x: CalcitValue): number => {
  // based on order used in Ord traint
  if (x == null) return PseudoTypeIndex.nil;
  let t = typeof x;
  if (t === "boolean") return PseudoTypeIndex.bool;
  if (t === "number") return PseudoTypeIndex.number;
  if (x instanceof CalcitSymbol) return PseudoTypeIndex.symbol;
  if (x instanceof CalcitKeyword) return PseudoTypeIndex.keyword;
  if (t === "string") return PseudoTypeIndex.string;
  if (x instanceof CalcitRef) return PseudoTypeIndex.ref;
  if (x instanceof CalcitTuple) return PseudoTypeIndex.tuple;
  if (x instanceof CalcitRecur) return PseudoTypeIndex.recur;
  if (x instanceof CalcitList || x instanceof CalcitSliceList) return PseudoTypeIndex.list;
  if (x instanceof CalcitSet) return PseudoTypeIndex.set;
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) return PseudoTypeIndex.map;
  if (x instanceof CalcitRecord) return PseudoTypeIndex.record;
  // proc, fn, macro, syntax, not distinguished
  if (t === "function") return PseudoTypeIndex.fn;
  throw new Error("unknown type to compare");
};

let rawCompare = (x: any, y: any): number => {
  if (x < y) {
    return -1;
  } else if (x > y) {
    return 1;
  } else {
    return 0;
  }
};

export let _$n_compare = (a: CalcitValue, b: CalcitValue): number => {
  if (a === b) return 0;
  let ta = typeAsInt(a);
  let tb = typeAsInt(b);
  if (ta === tb) {
    switch (ta) {
      case PseudoTypeIndex.nil:
        return 0;
      case PseudoTypeIndex.bool:
        return rawCompare(a, b);
      case PseudoTypeIndex.number:
        return rawCompare(a, b);
      case PseudoTypeIndex.keyword:
        return rawCompare((a as CalcitKeyword).value, (b as CalcitKeyword).value);
      case PseudoTypeIndex.symbol:
        return rawCompare(a, b);
      case PseudoTypeIndex.string:
        return rawCompare(a, b);
      case PseudoTypeIndex.ref:
        return rawCompare((a as CalcitRef).path, (b as CalcitRef).path);
      default:
        // TODO, need more accurate solution
        if (a < b) {
          return -1;
        } else if (a > b) {
          return 1;
        } else {
          return 0;
        }
    }
  } else {
    return rawCompare(ta, tb);
  }
};
