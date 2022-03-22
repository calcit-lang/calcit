import { initTernaryTreeMap, Hash, insert } from "@calcit/ternary-tree";
import { CalcitValue } from "./js-primes.mjs";
import { kwd, castKwd, toString, CalcitKeyword, getStringName, findInFields } from "./calcit-data.mjs";

import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";

export class CalcitRecord {
  name: CalcitKeyword;
  fields: Array<CalcitKeyword>;
  values: Array<CalcitValue>;
  cachedHash: Hash;
  constructor(name: CalcitKeyword, fields: Array<CalcitKeyword>, values?: Array<CalcitValue>) {
    this.name = name;
    let fieldNames = fields.map(castKwd);
    this.fields = fields;
    if (values != null) {
      if (values.length !== fields.length) {
        throw new Error("fields/values length not match");
      }
      this.values = values;
    } else {
      this.values = new Array(fieldNames.length);
    }
    this.cachedHash = null;
  }
  get(k: CalcitValue) {
    let field = castKwd(k);
    let idx = findInFields(this.fields, field);
    if (idx >= 0) {
      return this.values[idx];
    } else {
      throw new Error(`Cannot find :${field} among (${this.fields.join(",")})`);
    }
  }
  assoc(k: CalcitValue, v: CalcitValue): CalcitRecord {
    let values: Array<CalcitValue> = new Array(this.fields.length);
    let k_id = castKwd(k);
    for (let idx = 0; idx < this.fields.length; idx++) {
      if (this.fields[idx] === k_id) {
        values[idx] = v;
      } else {
        values[idx] = this.values[idx];
      }
    }
    return new CalcitRecord(this.name, this.fields, values);
  }
  /** return -1 for missing */
  findIndex(k: CalcitValue) {
    let field = castKwd(k);
    let idx = findInFields(this.fields, field);
    return idx;
  }
  contains(k: CalcitValue) {
    let idx = this.findIndex(k);
    return idx >= 0;
  }
  toString(): string {
    let ret = "(%{} " + this.name;
    for (let idx = 0; idx < this.fields.length; idx++) {
      ret += " (" + this.fields[idx] + " " + toString(this.values[idx], true) + ")";
    }
    return ret + ")";
  }
}

export let new_record = (name: CalcitValue, ...fields: Array<CalcitValue>): CalcitValue => {
  let fieldNames = fields.map(castKwd).sort((x, y) => {
    if (x.idx < y.idx) {
      return -1;
    } else if (x.idx > y.idx) {
      return 1;
    } else {
      throw new Error(`Unexpected duplication in record fields: ${x.toString()}`);
    }
  });
  return new CalcitRecord(castKwd(name), fieldNames);
};

export let fieldsEqual = (xs: Array<CalcitKeyword>, ys: Array<CalcitKeyword>): boolean => {
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

export let _$n__PCT__$M_ = (proto: CalcitValue, ...xs: Array<CalcitValue>): CalcitValue => {
  if (proto instanceof CalcitRecord) {
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
        if (k === castKwd(xs[j * 2])) {
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

    return new CalcitRecord(proto.name, proto.fields, values);
  } else {
    throw new Error("Expected prototype to be a record");
  }
};

export let _$n_record_$o_get_name = (x: CalcitRecord): CalcitKeyword => {
  if (x instanceof CalcitRecord) {
    return x.name;
  } else {
    throw new Error("Expected a record");
  }
};

export let _$n_record_$o_from_map = (proto: CalcitValue, data: CalcitValue): CalcitValue => {
  if (!(proto instanceof CalcitRecord)) throw new Error("Expected prototype to be record");

  if (data instanceof CalcitRecord) {
    if (fieldsEqual(proto.fields, data.fields)) {
      return new CalcitRecord(proto.name, proto.fields, data.values);
    } else {
      let values: Array<CalcitValue> = [];
      for (let field of proto.fields) {
        let idx = findInFields(data.fields, field);
        if (idx < 0) {
          throw new Error(`Cannot find field ${field} among ${data.fields}`);
        }
        values.push(data.values[idx]);
      }
      return new CalcitRecord(proto.name, proto.fields, values);
    }
  } else if (data instanceof CalcitMap || data instanceof CalcitSliceMap) {
    let pairs: Array<[CalcitKeyword, CalcitValue]> = [];
    for (let [k, v] of data.pairs()) {
      pairs.push([castKwd(k), v]);
    }
    // mutable sort
    pairs.sort((pair1, pair2) => pair1[0].cmp(pair2[0]));

    let values: Array<CalcitValue> = [];
    outerLoop: for (let field of proto.fields) {
      for (let idx = 0; idx < pairs.length; idx++) {
        let pair = pairs[idx];
        if (pair[0] === field) {
          values.push(pair[1]);
          continue outerLoop; // dirty code for performance
        }
      }
      throw new Error(`Cannot find field ${field} among ${pairs}`);
    }
    return new CalcitRecord(proto.name, proto.fields, values);
  } else {
    throw new Error("Expected record or data for making a record");
  }
};

export let _$n_record_$o_to_map = (x: CalcitValue): CalcitValue => {
  if (x instanceof CalcitRecord) {
    var dict: Array<CalcitValue> = [];
    for (let idx = 0; idx < x.fields.length; idx++) {
      dict.push(x.fields[idx], x.values[idx]);
    }
    return new CalcitSliceMap(dict);
  } else {
    throw new Error("Expected record");
  }
};

export let _$n_record_$o_matches_$q_ = (x: CalcitValue, y: CalcitValue): boolean => {
  if (!(x instanceof CalcitRecord)) {
    throw new Error("Expected first argument to be record");
  }
  if (!(y instanceof CalcitRecord)) {
    throw new Error("Expected second argument to be record");
  }

  if (x.name !== y.name) {
    return false;
  }
  return fieldsEqual(x.fields, y.fields);
};

export function _$n_record_$o_extend_as(obj: CalcitValue, new_name: CalcitValue, new_key: CalcitValue, new_value: CalcitValue) {
  if (arguments.length !== 4) throw new Error(`Expected 4 arguments, got ${arguments.length}`);
  if (!(obj instanceof CalcitRecord)) throw new Error("Expected record");
  let field = castKwd(new_key);
  let new_name_kwd = castKwd(new_name);
  let new_fields: CalcitKeyword[] = [];
  let new_values: CalcitValue[] = [];
  let inserted = false;

  for (let i = 0; i < new_fields.length; i++) {
    let k = new_fields[i];
    if (inserted) {
      new_fields.push(k);
      new_values.push(obj.values[i]);
    } else {
      let ordering = field.cmp(k);
      if (ordering === -1) {
        new_fields.push(field);
        new_values.push(new_value);

        new_fields.push(k);
        new_values.push(obj.values[i]);
      } else if (ordering === 1) {
        new_fields.push(k);
        new_values.push(obj.values[i]);
      } else {
        throw new Error("Does not extend existed record field");
      }
    }
  }
  if (!inserted) {
    new_fields.push(field);
    new_values.push(new_value);
  }

  return new CalcitRecord(new_name_kwd, new_fields, new_values);
}
