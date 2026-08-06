import { initTernaryTreeMap, Hash, insert } from "@calcit/ternary-tree";
import { CalcitValue } from "./js-primes.mjs";
import { CalcitImpl } from "./js-impl.mjs";
import { newTag, castTag, toString, CalcitTag, getStringName, findInFields } from "./calcit-data.mjs";

import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";

import { CalcitStruct } from "./js-struct.mjs";

export class CalcitRecord {
  name: CalcitTag;
  fields: Array<CalcitTag>;
  values: Array<CalcitValue>;
  structRef: CalcitStruct;
  cachedHash: Hash;
  constructor(name: CalcitTag, fields: Array<CalcitTag>, values?: Array<CalcitValue>, structRef?: CalcitStruct) {
    this.name = name;
    let fieldNames = fields.map(castTag);
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
    this.structRef = structRef || new CalcitStruct(name, fields, new Array(fields.length).fill(null));
  }
  get(k: CalcitValue) {
    let field = castTag(k);
    let idx = findInFields(this.fields, field);
    if (idx >= 0) {
      return this.values[idx];
    } else {
      return undefined;
    }
  }
  getOrNil(k: CalcitValue) {
    let field = castTag(k);
    let idx = findInFields(this.fields, field);
    if (idx >= 0) {
      return this.values[idx];
    } else {
      return undefined;
    }
  }
  assoc(k: CalcitValue, v: CalcitValue): CalcitRecord {
    let values: Array<CalcitValue> = new Array(this.fields.length);
    let k_id = castTag(k);
    for (let idx = 0; idx < this.fields.length; idx++) {
      if (this.fields[idx] === k_id) {
        values[idx] = v;
      } else {
        values[idx] = this.values[idx];
      }
    }
    return new CalcitRecord(this.name, this.fields, values, this.structRef);
  }
  /** return -1 for missing */
  findIndex(k: CalcitValue) {
    let field = castTag(k);
    let idx = findInFields(this.fields, field);
    return idx;
  }
  contains(k: CalcitValue) {
    let idx = this.findIndex(k);
    return idx >= 0;
  }
  toString(disableJsDataWarning: boolean = false): string {
    // Optimize string building using array join instead of concatenation
    const parts = ["(%{} '", this.name.value];
    for (let idx = 0; idx < this.fields.length; idx++) {
      parts.push(" (", this.fields[idx].toString(), " ", toString(this.values[idx], true, disableJsDataWarning), ")");
    }
    parts.push(")");
    return parts.join("");
  }
  withImpls(impl: CalcitValue | CalcitImpl[]): CalcitRecord {
    let nextImpls: CalcitImpl[];
    if (impl instanceof CalcitImpl) {
      nextImpls = [impl];
    } else if (Array.isArray(impl)) {
      nextImpls = impl;
    } else {
      throw new Error("Expected an impl or array of impls");
    }
    let nextStruct = new CalcitStruct(this.name, this.fields, this.structRef.fieldTypes, this.structRef.impls.concat(nextImpls));
    return new CalcitRecord(this.name, this.fields, this.values, nextStruct);
  }
}

export let new_record = (name: CalcitValue, ...fields: Array<CalcitValue>): CalcitValue => {
  let fieldNames = fields.map(castTag).sort((x, y) => {
    if (x.idx < y.idx) {
      return -1;
    } else if (x.idx > y.idx) {
      return 1;
    } else {
      throw new Error(`Unexpected duplication in record fields: ${x.toString()}`);
    }
  });
  return new CalcitRecord(castTag(name), fieldNames);
};

export let new_impl_record = (impl: CalcitImpl, name: CalcitValue, ...fields: Array<CalcitValue>): CalcitValue => {
  let fieldNames = fields.map(castTag).sort((x, y) => {
    if (x.idx < y.idx) {
      return -1;
    } else if (x.idx > y.idx) {
      return 1;
    } else {
      throw new Error(`Unexpected duplication in record fields: ${x.toString()}`);
    }
  });
  let nameTag = castTag(name);
  let structRef = new CalcitStruct(nameTag, fieldNames, new Array(fieldNames.length).fill(null), [impl]);
  return new CalcitRecord(nameTag, fieldNames, undefined, structRef);
};

/** Loose record: `?{} :field1 val1 :field2 val2` – record without a declared struct.
 *  Fields are sorted by tag index. The record name is "?" (sentinel). */
export let _$q__$M_ = (...xs: Array<CalcitValue>): CalcitValue => {
  if (xs.length % 2 !== 0) {
    throw new Error("?{} expected pairs of :field value");
  }
  let pairs: Array<[CalcitTag, CalcitValue]> = [];
  for (let i = 0; i < xs.length; i += 2) {
    pairs.push([castTag(xs[i]), xs[i + 1]]);
  }
  pairs.sort((a, b) => a[0].idx - b[0].idx);
  // Check for duplicate fields after sorting
  for (let i = 1; i < pairs.length; i++) {
    if (pairs[i][0].idx === pairs[i - 1][0].idx) {
      throw new Error(`?{} received duplicate field: :${getStringName(pairs[i][0])}`);
    }
  }
  let fieldNames = pairs.map((p) => p[0]);
  let values = pairs.map((p) => p[1]);
  let looseTag = newTag("?");
  return new CalcitRecord(looseTag, fieldNames, values);
};

export let fieldsEqual = (xs: Array<CalcitTag>, ys: Array<CalcitTag>): boolean => {
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
  let recordProto: CalcitRecord;
  if (proto instanceof CalcitRecord) {
    recordProto = proto;
  } else if (proto instanceof CalcitStruct) {
    recordProto = new CalcitRecord(proto.name, proto.fields, new Array(proto.fields.length).fill(null), proto);
  } else {
    throw new Error("Expected prototype to be a record");
  }
  {
    if (xs.length % 2 !== 0) {
      throw new Error("Expected even number of key/value");
    }
    if (xs.length !== recordProto.fields.length * 2) {
      throw new Error("fields size does not match");
    }

    let values = new Array(recordProto.fields.length);

    for (let i = 0; i < recordProto.fields.length; i++) {
      let idx = -1;
      let k = recordProto.fields[i];
      for (let j = 0; j < recordProto.fields.length; j++) {
        if (k === castTag(xs[j * 2])) {
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

    return new CalcitRecord(recordProto.name, recordProto.fields, values, recordProto.structRef);
  }
};

export let _$n__PCT__$M__$q_ = (proto: CalcitValue, ...xs: Array<CalcitValue>): CalcitValue => {
  let recordProto: CalcitRecord;
  let values: Array<CalcitValue>;
  if (proto instanceof CalcitStruct) {
    recordProto = new CalcitRecord(proto.name, proto.fields, new Array(proto.fields.length).fill(null), proto);
    values = recordProto.values.slice();
  } else {
    throw new Error("Expected prototype to be a struct");
  }

  if (xs.length % 2 !== 0) {
    throw new Error("Expected even number of key/value");
  }

  let touched = new Set<number>();
  for (let i = 0; i < xs.length; i += 2) {
    let k = castTag(xs[i]);
    let idx = findInFields(recordProto.fields, k);
    if (idx < 0) {
      throw new Error(`Cannot find field ${k} among ${recordProto.fields}`);
    }
    if (touched.has(idx)) {
      throw new Error(`record field already has value, probably duplicated key: ${k}`);
    }
    touched.add(idx);
    values[idx] = xs[i + 1];
  }

  return new CalcitRecord(recordProto.name, recordProto.fields, values, recordProto.structRef);
};

/// update record with new values
export let _$n_record_$o_with = (proto: CalcitValue, ...xs: Array<CalcitValue>): CalcitValue => {
  if (proto instanceof CalcitRecord) {
    if (xs.length % 2 !== 0) {
      throw new Error("Expected even number of key/value");
    }
    let values = proto.values.slice();
    for (let i = 0; i < xs.length; i += 2) {
      let k = castTag(xs[i]);
      let v = xs[i + 1];
      let idx = findInFields(proto.fields, k);
      if (idx < 0) {
        throw new Error(`Cannot find field ${k} among ${proto.fields}`);
      }
      values[idx] = v;
    }
    return new CalcitRecord(proto.name, proto.fields, values, proto.structRef);
  } else {
    throw new Error("Expected prototype to be a record");
  }
};

export let _$n_record_$o_get_name = (x: CalcitValue): CalcitTag => {
  if (x instanceof CalcitRecord) {
    return x.name;
  } else {
    throw new Error("Expected a record");
  }
};

export let _$n_record_$o_struct = (x: CalcitValue): CalcitValue => {
  if (x instanceof CalcitRecord) {
    return x.structRef ?? null;
  } else {
    throw new Error("Expected a record");
  }
};

export let _$n_record_$o_from_map = (proto: CalcitValue, data: CalcitValue): CalcitValue => {
  let recordProto: CalcitRecord;
  if (proto instanceof CalcitStruct) {
    recordProto = new CalcitRecord(proto.name, proto.fields, new Array(proto.fields.length).fill(null), proto);
  } else {
    throw new Error("Expected prototype to be struct");
  }

  if (data instanceof CalcitRecord) {
    if (fieldsEqual(recordProto.fields, data.fields)) {
      return new CalcitRecord(recordProto.name, recordProto.fields, data.values, recordProto.structRef);
    } else {
      let values: Array<CalcitValue> = [];
      for (let i = 0; i < recordProto.fields.length; i++) {
        let field = recordProto.fields[i];
        let idx = findInFields(data.fields, field);
        if (idx < 0) {
          throw new Error(`Cannot find field ${field} among ${data.fields}`);
        }
        values.push(data.values[idx]);
      }
      return new CalcitRecord(recordProto.name, recordProto.fields, values, recordProto.structRef);
    }
  } else if (data instanceof CalcitMap || data instanceof CalcitSliceMap) {
    let pairs_buffer: Array<[CalcitTag, CalcitValue]> = [];
    let pairs = data.pairs();
    for (let i = 0; i < pairs.length; i++) {
      let k = pairs[i][0];
      let v = pairs[i][1];
      pairs_buffer.push([castTag(k), v]);
    }
    // mutable sort
    pairs_buffer.sort((pair1, pair2) => pair1[0].cmp(pair2[0]));

    let values: Array<CalcitValue> = [];
    outerLoop: for (let i = 0; i < recordProto.fields.length; i++) {
      let field = recordProto.fields[i];
      for (let idx = 0; idx < pairs_buffer.length; idx++) {
        let pair = pairs_buffer[idx];
        if (pair[0] === field) {
          values.push(pair[1]);
          continue outerLoop; // dirty code for performance
        }
      }
      throw new Error(`Cannot find field ${field} among ${pairs_buffer}`);
    }
    return new CalcitRecord(recordProto.name, recordProto.fields, values, recordProto.structRef);
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
  let targetStruct: CalcitStruct;
  if (y instanceof CalcitRecord) {
    targetStruct = y.structRef;
  } else if (y instanceof CalcitStruct) {
    targetStruct = y;
  } else {
    throw new Error("Expected second argument to be record or struct");
  }

  if (x instanceof CalcitRecord) {
    if (x.name !== targetStruct.name) {
      return false;
    }
    return fieldsEqual(x.fields, targetStruct.fields);
  } else {
    throw new Error("Expected first argument to be record");
  }
};

export function _$n_record_$o_extend_as(obj: CalcitValue, new_name: CalcitValue, new_key: CalcitValue, new_value: CalcitValue) {
  if (arguments.length !== 4) throw new Error(`Expected 4 arguments, got ${arguments.length}`);
  if (!(obj instanceof CalcitRecord)) throw new Error("Expected record");
  let field = castTag(new_key);
  let new_name_tag = castTag(new_name);
  let new_fields: CalcitTag[] = [];
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

  return new CalcitRecord(new_name_tag, new_fields, new_values);
}
