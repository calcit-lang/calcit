import { CalcitTag, CalcitSymbol, CalcitRef, CalcitFn, CalcitRecur } from "./calcit-data.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";
import { CalcitRecord } from "./js-record.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitSet as CalcitSet } from "./js-set.mjs";
import { CalcitTuple } from "./js-tuple.mjs";
import { CalcitCirruQuote, cirru_deep_equal } from "./js-cirru.mjs";

export type CalcitValue =
  | string
  | number
  | boolean
  | CalcitMap
  | CalcitSliceMap
  | CalcitList
  | CalcitSliceList
  | CalcitSet
  | CalcitTag
  | CalcitSymbol
  | CalcitRef
  | CalcitTuple
  | CalcitFn
  | CalcitRecur // should not be exposed to function
  | CalcitRecord
  | CalcitCirruQuote
  | null;

export let is_literal = (x: CalcitValue): boolean => {
  if (x == null) return true;
  if (typeof x == "string") return true;
  if (typeof x == "boolean") return true;
  if (typeof x == "number") return true;
  if (x instanceof CalcitTag) return true;
  if (x instanceof CalcitSymbol) return true;
  return false;
};

enum PseudoTypeIndex {
  nil,
  bool,
  number,
  symbol,
  tag,
  string,
  ref,
  tuple,
  recur,
  list,
  set,
  map,
  record,
  fn,
  cirru_quote,
}

let typeAsInt = (x: CalcitValue): number => {
  // based on order used in Ord traint
  if (x == null) return PseudoTypeIndex.nil;
  let t = typeof x;
  if (t === "boolean") return PseudoTypeIndex.bool;
  if (t === "number") return PseudoTypeIndex.number;
  if (x instanceof CalcitSymbol) return PseudoTypeIndex.symbol;
  if (x instanceof CalcitTag) return PseudoTypeIndex.tag;
  if (t === "string") return PseudoTypeIndex.string;
  if (x instanceof CalcitRef) return PseudoTypeIndex.ref;
  if (x instanceof CalcitTuple) return PseudoTypeIndex.tuple;
  if (x instanceof CalcitRecur) return PseudoTypeIndex.recur;
  if (x instanceof CalcitList || x instanceof CalcitSliceList) return PseudoTypeIndex.list;
  if (x instanceof CalcitSet) return PseudoTypeIndex.set;
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) return PseudoTypeIndex.map;
  if (x instanceof CalcitRecord) return PseudoTypeIndex.record;
  if (x instanceof CalcitCirruQuote) return PseudoTypeIndex.cirru_quote;
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
      case PseudoTypeIndex.tag:
        return rawCompare((a as CalcitTag).value, (b as CalcitTag).value);
      case PseudoTypeIndex.symbol:
        return rawCompare(a, b);
      case PseudoTypeIndex.string:
        return rawCompare(a, b);
      case PseudoTypeIndex.ref:
        return rawCompare((a as CalcitRef).path, (b as CalcitRef).path);
      case PseudoTypeIndex.cirru_quote:
        return rawCompare(a, b); // TODO not stable
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
