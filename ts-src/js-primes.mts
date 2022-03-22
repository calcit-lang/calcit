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
