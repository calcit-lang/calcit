import { initTernaryTreeMap, valueHash } from "@calcit/ternary-tree";
import { CrDataValue } from "./js-primes";
import { kwd, toString, getStringName, findInFields } from "./calcit-data";

import { CrDataMap } from "./js-map";

export class CrDataRecord {
  name: string;
  fields: Array<string>;
  values: Array<CrDataValue>;
  constructor(name: string, fields: Array<CrDataValue>, values?: Array<CrDataValue>) {
    this.name = name;
    let fieldNames = fields.map(getStringName);
    this.fields = fieldNames;
    if (values != null) {
      if (values.length != fields.length) {
        throw new Error("value length not match");
      }
      this.values = values;
    } else {
      this.values = new Array(fieldNames.length);
    }
  }
  get(k: CrDataValue) {
    let field = getStringName(k);
    let idx = findInFields(this.fields, field);
    if (idx >= 0) {
      return this.values[idx];
    } else {
      throw new Error(`Cannot find :${field} among (${this.values.join(",")})`);
    }
  }
  assoc(k: CrDataValue, v: CrDataValue): CrDataRecord {
    let values: Array<CrDataValue> = new Array(this.fields.length);
    let name = getStringName(k);
    for (let idx in this.fields) {
      if (this.fields[idx] === name) {
        values[idx] = v;
      } else {
        values[idx] = this.values[idx];
      }
    }
    return new CrDataRecord(this.name, this.fields, values);
  }
  merge() {
    // TODO
  }
  contains(k: CrDataValue) {
    let field = getStringName(k);
    let idx = findInFields(this.fields, field);
    return idx >= 0;
  }
  toString(): string {
    let ret = "(%{} " + this.name;
    for (let idx in this.fields) {
      ret += " (" + this.fields[idx] + " " + toString(this.values[idx], true) + ")";
    }
    return ret + ")";
  }
}

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
  for (let idx = 0; idx < xs.length; idx++) {
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
    if (xs.length !== proto.fields.length * 2) {
      throw new Error("fields size does not match");
    }

    let values = new Array(proto.fields.length);

    for (let i = 0; i < proto.fields.length; i++) {
      let idx = -1;
      let k = proto.fields[i];
      for (let j = 0; j < proto.fields.length; j++) {
        if (k === getStringName(xs[j * 2])) {
          idx = j;
          break;
        }
      }

      if (idx < 0) {
        throw new Error("invalid field name for this record");
      }
      if (values[i] != null) {
        throw new Error("record field already has value, probably duplicated key");
      }
      values[i] = xs[idx * 2 + 1];
    }

    return new CrDataRecord(proto.name, proto.fields, values);
  } else {
    throw new Error("Expected prototype to be a record");
  }
};

export let get_record_name = (x: CrDataRecord): string => {
  if (x instanceof CrDataRecord) {
    return x.name;
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
