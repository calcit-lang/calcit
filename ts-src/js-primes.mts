import { CalcitTag, CalcitSymbol, CalcitFn, CalcitRecur, compareUnicodeStrings } from "./calcit-data.mjs";
import { CalcitRef } from "./js-ref.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";
import { CalcitStructValue } from "./js-struct-value.mjs";
import { CalcitImpl } from "./js-impl.mjs";
import { CalcitStructDef } from "./js-struct-def.mjs";
import { CalcitEnumDef } from "./js-enum-def.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitSet as CalcitSet } from "./js-set.mjs";
import { CalcitEnumValue } from "./js-enum-value.mjs";
import { CalcitCirruQuote, cirru_deep_equal } from "./js-cirru.mjs";
import { CalcitTrait } from "./js-trait.mjs";

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
  | CalcitEnumValue
  | CalcitFn
  | CalcitRecur // should not be exposed to function
  | CalcitStructValue
  | CalcitImpl
  | CalcitTrait
  | CalcitStructDef
  | CalcitEnumDef
  | CalcitCirruQuote
  | Uint8Array
  | undefined
  | null;

export let isLiteral = (x: CalcitValue): boolean => {
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
  unit,
  bool,
  number,
  symbol,
  tag,
  string,
  ref,
  enum_value,
  recur,
  list,
  set,
  map,
  struct_value,
  impl,
  struct_def,
  enum_def,
  fn,
  cirru_quote,
}

let typeAsInt = (x: CalcitValue): number => {
  // based on order used in Ord trait
  if (x === null) return PseudoTypeIndex.nil;
  if (x === undefined) return PseudoTypeIndex.unit;
  let t = typeof x;
  if (t === "boolean") return PseudoTypeIndex.bool;
  if (t === "number") return PseudoTypeIndex.number;
  if (x instanceof CalcitSymbol) return PseudoTypeIndex.symbol;
  if (x instanceof CalcitTag) return PseudoTypeIndex.tag;
  if (t === "string") return PseudoTypeIndex.string;
  if (x instanceof CalcitRef) return PseudoTypeIndex.ref;
  if (x instanceof CalcitEnumValue) return PseudoTypeIndex.enum_value;
  if (x instanceof CalcitRecur) return PseudoTypeIndex.recur;
  if (x instanceof CalcitList || x instanceof CalcitSliceList) return PseudoTypeIndex.list;
  if (x instanceof CalcitSet) return PseudoTypeIndex.set;
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) return PseudoTypeIndex.map;
  if (x instanceof CalcitStructValue) return PseudoTypeIndex.struct_value;
  if (x instanceof CalcitImpl) return PseudoTypeIndex.impl;
  if (x instanceof CalcitStructDef) return PseudoTypeIndex.struct_def;
  if (x instanceof CalcitEnumDef) return PseudoTypeIndex.enum_def;
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

/** lexicographic order over two sequences of values, a shorter prefix sorts first (same as Rust `Vec::cmp`) */
let compareSequences = (xs: CalcitValue[], ys: CalcitValue[]): number => {
  let n = Math.min(xs.length, ys.length);
  for (let i = 0; i < n; i++) {
    let order = _$n_compare(xs[i], ys[i]);
    if (order !== 0) return order;
  }
  return rawCompare(xs.length, ys.length);
};

let listToArray = (x: CalcitList | CalcitSliceList): CalcitValue[] => Array.from(x.items());

export let _$n_compare = (a: CalcitValue, b: CalcitValue): number => {
  if (a === b) return 0;
  let ta = typeAsInt(a);
  let tb = typeAsInt(b);
  if (ta === tb) {
    switch (ta) {
      case PseudoTypeIndex.nil:
      case PseudoTypeIndex.unit:
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
      case PseudoTypeIndex.struct_value: {
        const left = a as CalcitStructValue;
        const right = b as CalcitStructValue;
        const nameOrder = compareUnicodeStrings(left.name.value, right.name.value);
        if (nameOrder !== 0) return nameOrder;
        const leftRef = left.structRef.definitionRef;
        const rightRef = right.structRef.definitionRef;
        if (leftRef !== rightRef) {
          if (leftRef == null) return -1;
          if (rightRef == null) return 1;
          return compareUnicodeStrings(leftRef, rightRef);
        }
        for (let index = 0; index < Math.min(left.fields.length, right.fields.length); index++) {
          const fieldOrder = compareUnicodeStrings(left.fields[index].value, right.fields[index].value);
          if (fieldOrder !== 0) return fieldOrder;
        }
        const fieldCountOrder = rawCompare(left.fields.length, right.fields.length);
        if (fieldCountOrder !== 0) return fieldCountOrder;
        for (let index = 0; index < Math.min(left.values.length, right.values.length); index++) {
          const valueOrder = _$n_compare(left.values[index], right.values[index]);
          if (valueOrder !== 0) return valueOrder;
        }
        return rawCompare(left.values.length, right.values.length);
      }
      case PseudoTypeIndex.cirru_quote:
        return rawCompare(a, b); // TODO not stable
      case PseudoTypeIndex.list:
        return compareSequences(listToArray(a as CalcitList), listToArray(b as CalcitList));
      case PseudoTypeIndex.enum_value: {
        const left = a as CalcitEnumValue;
        const right = b as CalcitEnumValue;
        const tagOrder = _$n_compare(left.tag, right.tag);
        if (tagOrder !== 0) return tagOrder;
        return compareSequences(left.extra, right.extra);
      }
      case PseudoTypeIndex.set: {
        // like native: smaller sets first, then the sorted elements
        const left = Array.from((a as CalcitSet).values());
        const right = Array.from((b as CalcitSet).values());
        const sizeOrder = rawCompare(left.length, right.length);
        if (sizeOrder !== 0) return sizeOrder;
        return compareSequences(left.sort(_$n_compare), right.sort(_$n_compare));
      }
      case PseudoTypeIndex.map: {
        // like native: smaller maps first, then the (key, value) pairs sorted by key and value
        const sortPairs = (pairs: Array<[CalcitValue, CalcitValue]>) =>
          pairs.sort((x, y) => _$n_compare(x[0], y[0]) || _$n_compare(x[1], y[1]));
        const left = sortPairs((a as CalcitMap).pairs());
        const right = sortPairs((b as CalcitMap).pairs());
        const sizeOrder = rawCompare(left.length, right.length);
        if (sizeOrder !== 0) return sizeOrder;
        for (let i = 0; i < left.length; i++) {
          const order = _$n_compare(left[i][0], right[i][0]) || _$n_compare(left[i][1], right[i][1]);
          if (order !== 0) return order;
        }
        return 0;
      }
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
