import { initTernaryTreeMap, valueHash } from "@calcit/ternary-tree";
import {
  CrDataSymbol,
  CrDataValue,
  CrDataKeyword,
  CrDataList,
  CrDataMap,
  CrDataAtom,
  CrDataFn,
  CrDataRecur,
  kwd,
  atomsRegistry,
  toString,
  CrDataSet,
  cloneSet,
  getStringName,
  CrDataRecord,
} from "./calcit-data";

export let new_record = (name: CrDataValue, ...fields: Array<CrDataValue>): CrDataValue => {
  let fieldNames = fields.map(getStringName).sort();
  return new CrDataRecord(getStringName(name), fieldNames);
};

let fieldPairOrder = (a: [string, CrDataValue], b: [string, CrDataValue]) => {
  if (a[0] < b[0]) {
    return -1;
  } else if (a[0] > b[0]) {
    return 1;
  } else {
    return 0;
  }
};

export let fieldsEqual = (xs: Array<string>, ys: Array<string>): boolean => {
  if (xs === ys) {
    return true; // special case, referential equal
  }
  if (xs.length !== ys.length) {
    return false;
  }
  for (let idx in xs) {
    if (xs[idx] !== ys[idx]) {
      return false;
    }
  }
  return true;
};

export let _AND__PCT__MAP_ = (proto: CrDataValue, ...xs: Array<CrDataValue>): CrDataValue => {
  if (proto instanceof CrDataRecord) {
    if (xs.length % 2 !== 0) {
      throw new Error("Expected even number of key/value");
    }
    let pairs: Array<[string, CrDataValue]> = [];
    for (let i = 0; i < xs.length >> 1; i++) {
      let idx = i << 1;
      pairs.push([getStringName(xs[idx]), xs[idx + 1]]);
    }
    // mutable sort for perf
    pairs.sort(fieldPairOrder);
    let fields = pairs.map((ys) => ys[0]);
    if (!fieldsEqual(fields, proto.fields)) {
      throw new Error("Fields does not match prototype");
    }
    let values = pairs.map((ys) => ys[1]);
    return new CrDataRecord(proto.name, proto.fields, values);
  } else {
    throw new Error("Expected prototype to be a record");
  }
};

export let get_record_name = (x: CrDataRecord): CrDataSymbol => {
  if (x instanceof CrDataRecord) {
    return new CrDataSymbol(x.name);
  } else {
    throw new Error("Expected a record");
  }
};

export let make_record = (proto: CrDataValue, data: CrDataValue): CrDataValue => {
  if (proto instanceof CrDataRecord) {
    if (data instanceof CrDataRecord) {
      if (fieldsEqual(proto.fields, data.fields)) {
        return new CrDataRecord(proto.name, proto.fields, data.values);
      } else {
        let values: Array<CrDataValue> = [];
        for (let field of proto.fields) {
          let idx = data.fields.indexOf(field);
          if (idx < 0) {
            throw new Error(`Cannot find field ${field} among ${data.fields}`);
          }
          values.push(data.values[idx]);
        }
        return new CrDataRecord(proto.name, proto.fields, values);
      }
    } else if (data instanceof CrDataMap) {
      let pairs: Array<[string, CrDataValue]> = [];
      for (let [k, v] of data.pairs()) {
        pairs.push([getStringName(k), v]);
      }
      // mutable sort
      pairs.sort(fieldPairOrder);

      let values: Array<CrDataValue> = [];
      outerLoop: for (let field of proto.fields) {
        for (let pair of pairs) {
          if (pair[0] === field) {
            values.push(pair[1]);
            continue outerLoop; // dirty code for performance
          }
        }
        throw new Error(`Cannot find field ${field} among ${pairs}`);
      }
      return new CrDataRecord(proto.name, proto.fields, values);
    } else {
      throw new Error("Expected record or data for making a record");
    }
  } else {
    throw new Error("Expected prototype to be record");
  }
};

export let turn_map = (x: CrDataValue): CrDataValue => {
  if (x instanceof CrDataRecord) {
    var dict: Array<[CrDataValue, CrDataValue]> = [];
    for (let idx in x.fields) {
      dict.push([kwd(x.fields[idx]), x.values[idx]]);
    }
    return new CrDataMap(initTernaryTreeMap(dict));
  } else {
    throw new Error("Expected record");
  }
};

export let relevant_record_QUES_ = (x: CrDataValue, y: CrDataValue): boolean => {
  if (!(x instanceof CrDataRecord)) {
    throw new Error("Expected record");
  }
  if (!(y instanceof CrDataRecord)) {
    throw new Error("Expected record");
  }

  if (x.name !== y.name) {
    return false;
  }
  return fieldsEqual(x.fields, y.fields);
};
