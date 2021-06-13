import { CalcitKeyword, CalcitSymbol as CalcitSymbol, CalcitRef, CalcitFn, CalcitRecur } from "./calcit-data";
import { CalcitList } from "./js-list";
import { CalcitRecord } from "./js-record";
import { CalcitMap } from "./js-map";
import { CalcitSet as CalcitSet } from "./js-set";
import { CalcitTuple } from "./js-tuple";

export type CalcitValue =
  | string
  | number
  | boolean
  | CalcitMap
  | CalcitList
  | CalcitSet
  | CalcitKeyword
  | CalcitSymbol
  | CalcitRef
  | CalcitTuple
  | CalcitFn
  | CalcitRecur // should not be exposed to function
  | CalcitRecord
  | null;
