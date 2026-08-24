import { initTernaryTreeMap, Hash, insert } from "@calcit/ternary-tree";
import { CalcitValue } from "./js-primes.mjs";
import { CalcitImpl } from "./js-impl.mjs";
import { newTag, castTag, toString, CalcitTag, getStringName, findInFields, compareTagNames } from "./calcit-data.mjs";

import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";

import { CalcitStructDef } from "./js-struct-def.mjs";

export class CalcitStructValue {
  name: CalcitTag;
  fields: Array<CalcitTag>;
  values: Array<CalcitValue>;
  structRef: CalcitStructDef;
  cachedHash: Hash;
  constructor(name: CalcitTag, fields: Array<CalcitTag>, values?: Array<CalcitValue>, structRef?: CalcitStructDef) {
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
    this.structRef = structRef || new CalcitStructDef(name, fields, new Array(fields.length).fill(null));
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
  getRequired(k: CalcitValue): CalcitValue {
    const value = this.get(k);
    if (value !== undefined) return value;
    throw new Error(`struct '${this.name.value}' does not define field ${castTag(k).toString()}`);
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
  assoc(k: CalcitValue, v: CalcitValue): CalcitStructValue {
    let values: Array<CalcitValue> = new Array(this.fields.length);
    let k_id = castTag(k);
    for (let idx = 0; idx < this.fields.length; idx++) {
      if (this.fields[idx] === k_id) {
        values[idx] = v;
      } else {
        values[idx] = this.values[idx];
      }
    }
    return new CalcitStructValue(this.name, this.fields, values, this.structRef);
  }
  nthAt(index: CalcitValue, field: CalcitValue): CalcitValue {
    const idx = checkedStructIndex(index, "&struct:nth");
    this.assertFieldAt(idx, field, "&struct:nth");
    return this.values[idx];
  }
  assocAt(index: CalcitValue, field: CalcitValue, value: CalcitValue): CalcitStructValue {
    const idx = checkedStructIndex(index, "&struct:assoc-at");
    this.assertFieldAt(idx, field, "&struct:assoc-at");
    const values = this.values.slice();
    values[idx] = value;
    return new CalcitStructValue(this.name, this.fields, values, this.structRef);
  }
  withAt(...triples: CalcitValue[]): CalcitStructValue {
    if (triples.length % 3 !== 0) {
      throw new Error("&struct:with-at expected index/tag/value triples");
    }
    const values = this.values.slice();
    for (let base = 0; base < triples.length; base += 3) {
      const idx = checkedStructIndex(triples[base], "&struct:with-at");
      this.assertFieldAt(idx, triples[base + 1], "&struct:with-at");
      values[idx] = triples[base + 2];
    }
    return new CalcitStructValue(this.name, this.fields, values, this.structRef);
  }
  private assertFieldAt(idx: number, field: CalcitValue, operation: string): void {
    if (idx >= this.fields.length) {
      throw new Error(`${operation} index ${idx} out of range for struct '${this.name.value}' with ${this.fields.length} fields`);
    }
    const fieldTag = castTag(field);
    const expectedTag = this.fields[idx];
    if (expectedTag.value !== fieldTag.value) {
      throw new Error(`${operation} index ${idx} expects field :${expectedTag.value}, but received :${fieldTag.value}`);
    }
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
    const parts = this.name.value === "_" ? ["(%{} _"] : ["(%{} '", this.name.value];
    for (let idx = 0; idx < this.fields.length; idx++) {
      parts.push(" (", this.fields[idx].toString(), " ", toString(this.values[idx], true, disableJsDataWarning), ")");
    }
    parts.push(")");
    return parts.join("");
  }
  withImpls(impl: CalcitValue | CalcitImpl[]): CalcitStructValue {
    let nextImpls: CalcitImpl[];
    if (impl instanceof CalcitImpl) {
      nextImpls = [impl];
    } else if (Array.isArray(impl)) {
      nextImpls = impl;
    } else {
      throw new Error("Expected an impl or array of impls");
    }
    let nextStruct = new CalcitStructDef(this.name, this.fields, this.structRef.fieldTypes, this.structRef.impls.concat(nextImpls));
    return new CalcitStructValue(this.name, this.fields, this.values, nextStruct);
  }
}

export let new_struct_value = (name: CalcitValue, ...fields: Array<CalcitValue>): CalcitValue => {
  let fieldNames = fields.map(castTag).sort(compareTagNames);
  assertUniqueFields(fieldNames, "Unexpected duplication in struct fields");
  return new CalcitStructValue(castTag(name), fieldNames);
};

export let new_impl_struct_value = (impl: CalcitImpl, name: CalcitValue, ...fields: Array<CalcitValue>): CalcitValue => {
  let fieldNames = fields.map(castTag).sort(compareTagNames);
  assertUniqueFields(fieldNames, "Unexpected duplication in struct fields");
  let nameTag = castTag(name);
  let structRef = new CalcitStructDef(nameTag, fieldNames, new Array(fieldNames.length).fill(null), [impl]);
  return new CalcitStructValue(nameTag, fieldNames, undefined, structRef);
};

/** Loose record: `?{} :field1 val1 :field2 val2` – record without a declared struct.
 *  Fields use the same lexical layout as named structs. The record name is "_". */
export let _$q__$M_ = (...xs: Array<CalcitValue>): CalcitValue => {
  if (xs.length % 2 !== 0) {
    throw new Error("?{} expected pairs of :field value");
  }
  let pairs: Array<[CalcitTag, CalcitValue]> = [];
  for (let i = 0; i < xs.length; i += 2) {
    pairs.push([castTag(xs[i]), xs[i + 1]]);
  }
  pairs.sort((a, b) => compareTagNames(a[0], b[0]));
  // Check for duplicate fields after sorting
  for (let i = 1; i < pairs.length; i++) {
    if (pairs[i][0].value === pairs[i - 1][0].value) {
      throw new Error(`?{} received duplicate field: :${getStringName(pairs[i][0])}`);
    }
  }
  let fieldNames = pairs.map((p) => p[0]);
  let values = pairs.map((p) => p[1]);
  let looseTag = newTag("_");
  return new CalcitStructValue(looseTag, fieldNames, values);
};

function checkedStructIndex(value: CalcitValue, operation: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${operation} expected a non-negative integer index, but received: ${value}`);
  }
  return value;
}

function assertUniqueFields(fields: CalcitTag[], message: string): void {
  for (let idx = 1; idx < fields.length; idx++) {
    if (fields[idx - 1].value === fields[idx].value) {
      throw new Error(`${message}: ${fields[idx].toString()}`);
    }
  }
}

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
  let recordProto: CalcitStructValue;
  if (proto instanceof CalcitStructValue) {
    recordProto = proto;
  } else if (proto instanceof CalcitStructDef) {
    recordProto = new CalcitStructValue(proto.name, proto.fields, new Array(proto.fields.length).fill(null), proto);
  } else {
    throw new Error("Expected prototype to be a StructDef");
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
        throw new Error("invalid field name for this struct");
      }
      if (values[i] != null) {
        throw new Error("struct field already has value, probably duplicated key");
      }
      values[i] = xs[idx * 2 + 1];
    }

    return new CalcitStructValue(recordProto.name, recordProto.fields, values, recordProto.structRef);
  }
};

export let _$n__PCT__$M__$q_ = (proto: CalcitValue, ...xs: Array<CalcitValue>): CalcitValue => {
  let recordProto: CalcitStructValue;
  let values: Array<CalcitValue>;
  if (proto instanceof CalcitStructDef) {
    recordProto = new CalcitStructValue(proto.name, proto.fields, new Array(proto.fields.length).fill(null), proto);
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
      throw new Error(`struct field already has value, probably duplicated key: ${k}`);
    }
    touched.add(idx);
    values[idx] = xs[i + 1];
  }

  return new CalcitStructValue(recordProto.name, recordProto.fields, values, recordProto.structRef);
};

/// update record with new values
export let _$n_struct_$o_with = (proto: CalcitValue, ...xs: Array<CalcitValue>): CalcitValue => {
  if (proto instanceof CalcitStructValue) {
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
    return new CalcitStructValue(proto.name, proto.fields, values, proto.structRef);
  } else {
    throw new Error("Expected prototype to be a StructDef");
  }
};

export let _$n_struct_$o_get_name = (x: CalcitValue): CalcitTag => {
  if (x instanceof CalcitStructValue) {
    return x.name;
  } else {
    throw new Error("Expected a struct value");
  }
};

export let _$n_struct_$o_definition = (x: CalcitValue): CalcitValue => {
  if (x instanceof CalcitStructValue) {
    if (x.name.value === "_") return null;
    return x.structRef ?? null;
  } else {
    throw new Error("Expected a struct value");
  }
};

export let _$n_struct_$o_from_map = (proto: CalcitValue, data: CalcitValue): CalcitValue => {
  let recordProto: CalcitStructValue;
  if (proto instanceof CalcitStructDef) {
    recordProto = new CalcitStructValue(proto.name, proto.fields, new Array(proto.fields.length).fill(null), proto);
  } else {
    throw new Error("Expected prototype to be struct");
  }

  if (data instanceof CalcitStructValue) {
    if (fieldsEqual(recordProto.fields, data.fields)) {
      return new CalcitStructValue(recordProto.name, recordProto.fields, data.values, recordProto.structRef);
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
      return new CalcitStructValue(recordProto.name, recordProto.fields, values, recordProto.structRef);
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
    return new CalcitStructValue(recordProto.name, recordProto.fields, values, recordProto.structRef);
  } else {
    throw new Error("Expected a struct value or data for making a struct");
  }
};

export let _$n_struct_$o_to_map = (x: CalcitValue): CalcitValue => {
  if (x instanceof CalcitStructValue) {
    var dict: Array<CalcitValue> = [];
    for (let idx = 0; idx < x.fields.length; idx++) {
      dict.push(x.fields[idx], x.values[idx]);
    }
    return new CalcitSliceMap(dict);
  } else {
    throw new Error("Expected a struct value");
  }
};

export let _$n_struct_$o_matches_$q_ = (x: CalcitValue, y: CalcitValue): boolean => {
  let targetStruct: CalcitStructDef;
  if (y instanceof CalcitStructValue) {
    targetStruct = y.structRef;
  } else if (y instanceof CalcitStructDef) {
    targetStruct = y;
  } else {
    throw new Error("Expected second argument to be a struct value or StructDef");
  }

  if (x instanceof CalcitStructValue) {
    if (x.name !== targetStruct.name) {
      return false;
    }
    return fieldsEqual(x.fields, targetStruct.fields);
  } else {
    throw new Error("Expected first argument to be a struct value");
  }
};

export function _$n_struct_$o_extend_as(obj: CalcitValue, new_name: CalcitValue, new_key: CalcitValue, new_value: CalcitValue) {
  if (arguments.length !== 4) throw new Error(`Expected 4 arguments, got ${arguments.length}`);
  if (!(obj instanceof CalcitStructValue)) throw new Error("Expected a struct value");
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
        throw new Error("Cannot extend an existing struct field");
      }
    }
  }
  if (!inserted) {
    new_fields.push(field);
    new_values.push(new_value);
  }

  return new CalcitStructValue(new_name_tag, new_fields, new_values);
}
