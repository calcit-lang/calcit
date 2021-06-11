import { CrDataKeyword, CrDataSymbol, CrDataRef, CrDataFn, CrDataRecur } from "./calcit-data";
import { CrDataList } from "./js-list";
import { CrDataRecord } from "./js-record";
import { CrDataMap } from "./js-map";
import { CrDataSet } from "./js-set";
import { CrDataTuple } from "./js-tuple";

export type CrDataValue =
  | string
  | number
  | boolean
  | CrDataMap
  | CrDataList
  | CrDataSet
  | CrDataKeyword
  | CrDataSymbol
  | CrDataRef
  | CrDataTuple
  | CrDataFn
  | CrDataRecur // should not be exposed to function
  | CrDataRecord
  | null;
