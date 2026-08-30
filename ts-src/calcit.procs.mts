import pkg from "./package.json" with { type: "json" };

export const calcit_version = pkg.version;
export const calcit_package_json = pkg;

import { parse, ICirruNode } from "@cirru/parser.ts";
import { writeCirruCode } from "@cirru/writer.ts";

import { CalcitValue } from "./js-primes.mjs";
import {
  CalcitSymbol,
  CalcitTag,
  CalcitFn,
  CalcitRecur,
  castTag,
  newTag,
  refsRegistry,
  toString,
  getStringName,
  compareTagNames,
  _$n__$e_,
  hashFunction,
} from "./calcit-data.mjs";

import { CalcitRef, atom } from "./js-ref.mjs";
import { CalcitStructValue } from "./js-struct-value.mjs";
import { CalcitImpl } from "./js-impl.mjs";
import { CalcitStructDef } from "./js-struct-def.mjs";
import { CalcitEnumDef } from "./js-enum-def.mjs";
import { CalcitTrait } from "./js-trait.mjs";

export * from "./calcit-data.mjs";
export * from "./js-struct-value.mjs";
export * from "./js-impl.mjs";
export * from "./js-struct-def.mjs";
export * from "./js-enum-def.mjs";
export * from "./js-map.mjs";
export * from "./js-list.mjs";
export * from "./js-set.mjs";
export * from "./js-primes.mjs";
export * from "./js-enum-value.mjs";
export * from "./js-trait.mjs";
export * from "./custom-formatter.mjs";
export * from "./js-cirru.mjs";
export * from "./js-arity-helpers.mjs";
export * from "./js-tag-helpers.mjs";
export * from "./js-buf-list.mjs";
export { _$n_compare } from "./js-primes.mjs";

import { CalcitList, CalcitSliceList, foldl } from "./js-list.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitSet } from "./js-set.mjs";
import { CalcitEnumValue } from "./js-enum-value.mjs";
import { to_calcit_data, extract_cirru_edn, extract_cirru_edn_for_typed, CalcitCirruQuote } from "./js-cirru.mjs";
import { TypedEdnSetView } from "./typed-edn.mjs";

let inNodeJs = typeof process !== "undefined" && process?.release?.name === "node";

export let type_of = (x: any): CalcitTag => {
  if (typeof x === "string") {
    return newTag("string");
  }
  if (typeof x === "number") {
    return newTag("number");
  }
  if (x instanceof CalcitTag) {
    return newTag("tag");
  }
  if (x instanceof CalcitList || x instanceof CalcitSliceList) {
    return newTag("list");
  }
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) {
    return newTag("map");
  }
  if (x === null) {
    return newTag("nil");
  }
  if (x === undefined) return newTag("unit");
  if (x instanceof CalcitRef) {
    return newTag("ref");
  }
  if (x instanceof CalcitEnumValue) {
    return newTag("enum");
  }
  if (x instanceof CalcitSymbol) {
    return newTag("symbol");
  }
  if (x instanceof CalcitSet) {
    return newTag("set");
  }
  if (x instanceof CalcitStructValue) {
    return newTag("struct");
  }
  if (x instanceof CalcitImpl) {
    return newTag("impl");
  }
  if (x instanceof CalcitStructDef) {
    return newTag("struct-def");
  }
  if (x instanceof CalcitEnumDef) {
    return newTag("enum-def");
  }
  if (x instanceof CalcitTrait) {
    return newTag("trait");
  }
  if (x instanceof CalcitCirruQuote) {
    return newTag("cirru-quote");
  }
  if (x === true || x === false) {
    return newTag("bool");
  }
  if (typeof x === "function") {
    if (x.isMacro) {
      // this is faked...
      return newTag("macro");
    }
    return newTag("fn");
  }
  if (typeof x === "object") {
    return newTag("js-object");
  }
  throw new Error(`Unknown data ${x}`);
};

const list_items = (item: CalcitValue): CalcitValue[] => {
  if (item instanceof CalcitList || item instanceof CalcitSliceList) {
    return Array.from(item.items());
  }
  throw new Error(`Expected list entry, got: ${item}`);
};

const isQuotedTypeVar = (item: CalcitValue): boolean => {
  if (item instanceof CalcitSymbol) {
    return true;
  }
  if (item instanceof CalcitList || item instanceof CalcitSliceList) {
    const items = Array.from(item.items());
    return items.length === 2 && items[0] instanceof CalcitSymbol && items[0].value === "quote" && items[1] instanceof CalcitSymbol;
  }
  return false;
};

const isGenericsEntry = (entry: CalcitValue): boolean => {
  if (!(entry instanceof CalcitList || entry instanceof CalcitSliceList)) {
    return false;
  }
  const items = Array.from(entry.items());
  return items.length > 0 && items.every(isQuotedTypeVar);
};

const isWhereMapEntry = (entry: CalcitValue): boolean => {
  // where-bound can be emitted as a CalcitMap (from _$n__$M_) or as a list-of-pairs
  if (entry instanceof CalcitMap || entry instanceof CalcitSliceMap) {
    return true;
  }
  if (!(entry instanceof CalcitList || entry instanceof CalcitSliceList)) {
    return false;
  }
  const items = Array.from(entry.items());
  if (items.length < 2) {
    return false;
  }
  const head = items[0];
  if (head instanceof CalcitTag || head instanceof CalcitSymbol || typeof head === "string") {
    return false;
  }
  return items.slice(1).every((item) => {
    if (!(item instanceof CalcitList || item instanceof CalcitSliceList)) {
      return false;
    }
    const pair = Array.from(item.items());
    return pair.length === 2;
  });
};

const trimDataDefinitionEntries = (entries: CalcitValue[]): CalcitValue[] => {
  let start = 0;
  if (entries[start] != null && isGenericsEntry(entries[start])) {
    start += 1;
  }
  if (entries[start] != null && isWhereMapEntry(entries[start])) {
    start += 1;
  }
  return entries.slice(start);
};

export let _$n_trait_$o__$o_new = function (name: CalcitValue, methods: CalcitValue): CalcitTrait {
  if (arguments.length !== 2) throw new Error("&trait::new expected 2 arguments");
  const items = list_items(methods);
  const methodNames: CalcitValue[] = [];
  const methodTypes: CalcitValue[] = [];
  for (let entry of items) {
    const pair = list_items(entry);
    if (pair.length !== 2) {
      throw new Error(`&trait::new expects (method type) pairs, got: ${toString(entry, true)}`);
    }
    methodNames.push(pair[0]);
    methodTypes.push(pair[1]);
  }
  return new CalcitTrait(name, methodNames, methodTypes);
};

export let _$n_assert_traits = function (value: CalcitValue, traitDef: CalcitValue): CalcitValue {
  if (arguments.length !== 2) throw new Error("&assert-traits expected 2 arguments");
  if (!(traitDef instanceof CalcitTrait)) {
    throw new Error(`&assert-traits expected a trait definition, but received: ${toString(traitDef, true)}`);
  }
  // Use the same merged builtin + attached impl list as method dispatch.
  // Otherwise records and tuples can call a builtin method while
  // `assert-traits` incorrectly claims that the corresponding trait is absent.
  const pair = lookup_impls(value);
  if (pair == null) {
    throw new Error(`&assert-traits cannot resolve impls for: ${toString(value, true)}`);
  }
  const impls = pair[0];
  const reverse =
    value instanceof CalcitStructValue || value instanceof CalcitEnumValue || value instanceof CalcitStructDef || value instanceof CalcitEnumDef;
  const ordered = reverse ? [...impls].reverse() : impls;
  const selected = ordered.find((impl) => impl != null && impl.origin === traitDef);
  if (selected == null) {
    const available = impls
      .filter((impl) => impl?.origin != null)
      .map((impl) => impl.origin!.name.toString())
      .join(" ");
    throw new Error(
      `assert-traits failed: ${toString(value, true)} does not nominally implement ${traitDef.toString()}. Available trait impls: ${
        available || "(none)"
      }`
    );
  }
  const missing = traitDef.methods.filter((method) => selected.getOrNil(method) == null);
  if (missing.length > 0) {
    throw new Error(
      `assert-traits failed: impl ${selected.name.toString()} for trait ${traitDef.name.toString()} is incomplete. Missing: ${missing.join(
        " "
      )}`
    );
  }
  return value;
};

export let defstruct = (name: CalcitValue, ...entries: CalcitValue[]): CalcitStructDef => {
  const structName = castTag(name);
  const fields: Array<{ tag: CalcitTag; type: CalcitValue }> = [];
  const fieldEntries = trimDataDefinitionEntries(entries);

  for (let entry of fieldEntries) {
    const items = list_items(entry);
    if (items.length !== 2) {
      throw new Error(`defstruct expects (field type) pairs, got: ${toString(entry, true)}`);
    }
    const fieldTag = castTag(items[0]);
    const fieldType = items[1];
    fields.push({ tag: fieldTag, type: fieldType });
  }

  fields.sort((a, b) => compareTagNames(a.tag, b.tag));
  for (let i = 1; i < fields.length; i++) {
    if (fields[i - 1].tag.value === fields[i].tag.value) {
      throw new Error(`defstruct duplicated field: ${fields[i].tag.toString()}`);
    }
  }

  const fieldTags = fields.map((entry) => entry.tag);
  const fieldTypes = fields.map((entry) => entry.type);
  return new CalcitStructDef(structName, fieldTags, fieldTypes, null);
};

export let defenum = (name: CalcitValue, ...variants: CalcitValue[]): CalcitEnumDef => {
  const enumName = castTag(name);
  const entries: Array<{ tag: CalcitTag; payload: CalcitSliceList }> = [];
  const variantEntries = trimDataDefinitionEntries(variants);

  for (let variant of variantEntries) {
    const items = list_items(variant);
    if (items.length === 0) {
      throw new Error("defenum expects variant tag and payload types, got empty list");
    }
    const tag = castTag(items[0]);
    const payload = new CalcitSliceList(items.slice(1));
    entries.push({ tag, payload });
  }

  entries.sort((a, b) => compareTagNames(a.tag, b.tag));
  for (let i = 1; i < entries.length; i++) {
    if (entries[i - 1].tag.value === entries[i].tag.value) {
      throw new Error(`defenum duplicated variant: ${entries[i].tag.toString()}`);
    }
  }

  const tags = entries.map((entry) => entry.tag);
  const values = entries.map((entry) => entry.payload);
  const prototype = new CalcitStructValue(enumName, tags, values, null);
  return new CalcitEnumDef(prototype);
};

export let _$n_impl_$o__$o_new = (name: CalcitValue, ...pairs: CalcitValue[]): CalcitImpl => {
  if (name === undefined) throw new Error("&impl::new expected arguments");
  const origin = name instanceof CalcitTrait ? name : null;
  const implName = origin ? origin.name : castTag(name);
  const entries: Array<{ tag: CalcitTag; value: CalcitValue }> = [];
  let sourcePairs = pairs;
  if (pairs.length === 1 && pairs[0] instanceof CalcitImpl) {
    const sourceImpl = pairs[0];
    sourcePairs = sourceImpl.fields.map(
      (field, idx) => new CalcitEnumValue(newTag(field.value), [sourceImpl.values[idx]], null)
    );
  }
  for (let idx = 0; idx < sourcePairs.length; idx++) {
    const pairValue = sourcePairs[idx];
    let fieldTag: CalcitTag;
    let value: CalcitValue;
    if (pairValue instanceof CalcitEnumValue) {
      if (pairValue.extra.length !== 1) {
        throw new Error(`&impl::new expects (field value) pairs, got: ${toString(pairValue, true)}`);
      }
      fieldTag = castTag(pairValue.tag);
      value = pairValue.extra[0];
    } else {
      const pair = list_items(pairValue);
      if (pair.length !== 2) {
        throw new Error(`&impl::new expects (field value) pairs, got: ${toString(pairValue, true)}`);
      }
      fieldTag = castTag(pair[0]);
      value = pair[1];
    }
    entries.push({ tag: fieldTag, value });
  }
  entries.sort((a, b) => compareTagNames(a.tag, b.tag));
  for (let i = 1; i < entries.length; i++) {
    if (entries[i - 1].tag.value === entries[i].tag.value) {
      throw new Error(`&impl::new duplicated field: ${entries[i].tag.toString()}`);
    }
  }
  const fields = entries.map((entry) => entry.tag);
  const values = entries.map((entry) => entry.value);
  if (origin != null) {
    const missing = origin.methods.filter((method) => !fields.some((field) => field.value === method.value));
    const unexpected = fields.filter((field) => !origin.methods.some((method) => method.value === field.value));
    if (missing.length > 0 || unexpected.length > 0) {
      const details: string[] = [];
      if (missing.length > 0) details.push(`missing methods: ${missing.join(" ")}`);
      if (unexpected.length > 0) details.push(`methods not declared by the trait: ${unexpected.join(" ")}`);
      throw new Error(`&impl::new does not conform to trait ${origin.name.toString()}: ${details.join("; ")}`);
    }
    for (const entry of entries) {
      if (typeof entry.value !== "function") {
        throw new Error(`&impl::new expects trait method .${entry.tag.value} to be a function, but received: ${toString(entry.value, true)}`);
      }
    }
  }
  return new CalcitImpl(implName, fields, values, origin);
};

export let _$n_struct_def_$o_new = (name: CalcitValue, ...entries: CalcitValue[]): CalcitStructDef => {
  return defstruct(name, ...entries);
};

export let _$n_enum_def_$o_new = (name: CalcitValue, ...variants: CalcitValue[]): CalcitEnumDef => {
  return defenum(name, ...variants);
};

export let print = (...xs: CalcitValue[]): void => {
  // TODO stringify each values
  console.log(xs.map((x) => toString(x, false)).join(" "));
};

export function _$n_list_$o_count(x: CalcitValue): number {
  if (x instanceof CalcitList || x instanceof CalcitSliceList) return x.len();

  throw new Error(`expected a list ${x}`);
}
export function _$n_str_$o_count(x: CalcitValue): number {
  if (typeof x === "string") {
    let count = 0;
    for (const _scalar of x) count += 1;
    return count;
  }

  throw new Error(`expected a string ${x}`);
}
export function _$n_str_$o_utf8_byte_count(x: CalcitValue): number {
  if (typeof x !== "string") throw new Error(`&str:utf8-byte-count expected a string ${x}`);

  let bytes = 0;
  for (let i = 0; i < x.length; i += 1) {
    const code = x.charCodeAt(i);
    if (code < 0x80) {
      bytes += 1;
    } else if (code < 0x800) {
      bytes += 2;
    } else if (code >= 0xd800 && code <= 0xdbff && i + 1 < x.length) {
      const next = x.charCodeAt(i + 1);
      if (next >= 0xdc00 && next <= 0xdfff) {
        bytes += 4;
        i += 1;
      } else {
        bytes += 3;
      }
    } else {
      bytes += 3;
    }
  }
  return bytes;
}
export function _$n_map_$o_count(x: CalcitValue): number {
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) return x.len();

  throw new Error(`expected a map ${x}`);
}
export function _$n_struct_$o_count(x: CalcitValue): number {
  if (x instanceof CalcitStructValue) return x.fields.length;

  throw new Error(`expected a struct value ${x}`);
}

export function _$n_struct_$o_field_tag(x: CalcitValue, idx: CalcitValue): CalcitTag {
  if (!(x instanceof CalcitStructValue)) throw new Error(`&struct:field-tag expected a struct value, got ${x}`);
  const i = idx as number;
  if (i < 0 || i >= x.fields.length) throw new Error(`&struct:field-tag index ${i} out of bounds (${x.fields.length})`);
  return x.fields[i];
}

export function _$n_struct_$o_nth(x: CalcitValue, idx: CalcitValue, field?: CalcitValue): CalcitValue {
  if (!(x instanceof CalcitStructValue)) throw new Error(`&struct:nth expected a struct value, got ${x}`);
  if (field !== undefined) return x.nthAt(idx, field);
  const i = idx as number;
  if (i < 0 || i >= x.values.length) throw new Error(`&struct:nth index ${i} out of range for struct with ${x.values.length} fields`);
  return x.values[i];
}

export function _$n_struct_$o_assoc_at(x: CalcitValue, idx: CalcitValue, field: CalcitValue, value: CalcitValue): CalcitValue {
  if (!(x instanceof CalcitStructValue)) throw new Error(`&struct:assoc-at expected a struct value, got ${x}`);
  return x.assocAt(idx, field, value);
}

export function _$n_struct_$o_with_at(x: CalcitValue, ...triples: CalcitValue[]): CalcitValue {
  if (!(x instanceof CalcitStructValue)) throw new Error(`&struct:with-at expected a struct value, got ${x}`);
  return x.withAt(...triples);
}


export function _$n_set_$o_count(x: CalcitValue): number {
  if (x instanceof CalcitSet) return x.len();

  throw new Error(`expected a set ${x}`);
}

export let _$L_ = (...xs: CalcitValue[]): CalcitSliceList => {
  return new CalcitSliceList(xs);
};
// single quote as alias for list
export let _SQUO_ = (...xs: CalcitValue[]): CalcitSliceList => {
  return new CalcitSliceList(xs);
};

export let _$n__$M_ = (...xs: CalcitValue[]): CalcitSliceMap => {
  if (xs.length % 2 !== 0) {
    throw new Error("&map expects even number of arguments");
  }
  return new CalcitSliceMap(xs);
};

export let defatom = (path: string, x: CalcitValue): CalcitValue => {
  let v = new CalcitRef(x, path);
  refsRegistry.set(path, v);
  return v;
};

export { atom } from "./js-ref.mjs";

export let peekDefatom = (path: string): CalcitRef => {
  return refsRegistry.get(path);
};

export let _$n_atom_$o_deref = (x: CalcitRef): CalcitValue => {
  if (x instanceof CalcitRef) {
    return x.value;
  } else {
    throw new Error("Expected CalcitRef");
  }
};

export let _$n__ADD_ = (x: number, y: number): number => {
  return x + y;
};

export let _$n__$s_ = (x: number, y: number): number => {
  return x * y;
};

export let _$n_str = (x: CalcitValue): string => {
  if (x === null) {
    return "";
  }
  if (typeof x === "string") {
    return x;
  }
  return toString(x, false);
};

export let _$n_str_$o_contains_$q_ = (xs: CalcitValue, x: CalcitValue): boolean => {
  if (typeof xs === "string") {
    if (typeof x != "number") {
      throw new Error("Expected number index for detecting");
    }
    let size = xs.length;
    if (x >= 0 && x < size) {
      return true;
    }
    return false;
  }

  throw new Error("string `contains?` expected a string");
};

export let _$n_list_$o_contains_$q_ = (xs: CalcitValue, x: CalcitValue): boolean => {
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    if (typeof x != "number") {
      throw new Error("Expected number index for detecting");
    }
    let size = xs.len();
    if (x >= 0 && x < size) {
      return true;
    }
    return false;
  }

  throw new Error("list `contains?` expected a list");
};

export let _$n_map_$o_contains_$q_ = (xs: CalcitValue, x: CalcitValue): boolean => {
  if (xs instanceof CalcitMap || xs instanceof CalcitSliceMap) return xs.contains(x);

  throw new Error("map `contains?` expected a map");
};

export let _$n_struct_$o_contains_$q_ = (xs: CalcitValue, x: CalcitValue): boolean => {
  if (xs instanceof CalcitStructValue) return xs.contains(x);

  throw new Error("struct `contains?` expected a struct value");
};

export let _$n_str_$o_includes_$q_ = (xs: CalcitValue, x: CalcitValue): boolean => {
  if (typeof xs === "string") {
    if (typeof x !== "string") {
      throw new Error("Expected string");
    }
    return xs.includes(x as string);
  }

  throw new Error("string includes? expected a string");
};

export let _$n_list_$o_includes_$q_ = (xs: CalcitValue, x: CalcitValue): boolean => {
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    let size = xs.len();
    for (let v of xs.items()) {
      if (_$n__$e_(v, x)) {
        return true;
      }
    }
    return false;
  }

  throw new Error("list includes? expected a list");
};

export let _$n_map_$o_includes_$q_ = (xs: CalcitValue, x: CalcitValue): boolean => {
  if (xs instanceof CalcitMap || xs instanceof CalcitSliceMap) {
    let pairs = xs.pairs();
    for (let idx = 0; idx < pairs.length; idx = idx + 1) {
      let v = pairs[idx][1];
      if (_$n__$e_(v, x)) {
        return true;
      }
    }
    return false;
  }

  throw new Error("map includes? expected a map");
};

export let _$n_set_$o_includes_$q_ = (xs: CalcitValue, x: CalcitValue): boolean => {
  if (xs instanceof CalcitSet) {
    return xs.contains(x);
  }

  throw new Error("set includes? expected a set");
};

export let _$n_str_$o_nth = function (xs: CalcitValue, k: CalcitValue) {
  if (arguments.length !== 2) throw new Error("nth takes 2 arguments");
  if (typeof k !== "number") throw new Error("Expected number index for a list");

  if (typeof xs === "string") return xs[k];

  throw new Error("Does not support `nth` on this type");
};

export let _$n_list_$o_nth = function (xs: CalcitValue, k: CalcitValue) {
  if (arguments.length !== 2) throw new Error("nth takes 2 arguments");
  if (typeof k !== "number") throw new Error("Expected number index for a list");

  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) return xs.get(k);

  throw new Error("Does not support `nth` on this type");
};

export let _$n_enum_$o_nth = function (xs: CalcitValue, k: CalcitValue) {
  if (arguments.length !== 2) throw new Error("nth takes 2 arguments");
  if (typeof k !== "number") throw new Error("Expected number index for a list");

  if (xs instanceof CalcitEnumValue) return xs.get(k);

  throw new Error("Does not support `nth` on this type");
};
export let _$n_enum_$o_count = function (xs: CalcitValue) {
  if (arguments.length !== 1) throw new Error("&enum:count takes 1 arguments");

  if (xs instanceof CalcitEnumValue) return xs.count();

  throw new Error("Does not support `count` on this type");
};

const coerce_impl = (value: CalcitValue, procName: string): CalcitImpl => {
  if (value instanceof CalcitImpl) {
    return value;
  }
  throw new Error(`${procName} expects trait impls as impls`);
};

export let _$n_enum_$o_impls = function (x: CalcitEnumValue) {
  if (arguments.length !== 1) throw new Error("&enum:impls takes 1 argument");
  return new CalcitSliceList(x.impls);
};

export let _$n_enum_$o_params = function (x: CalcitEnumValue) {
  if (arguments.length !== 1) throw new Error("&enum:params takes 1 argument");
  return new CalcitSliceList(x.extra);
};

export let _$n_enum_$o_with_impls = function (x: CalcitEnumValue, y: CalcitValue) {
  if (arguments.length !== 2) throw new Error("&enum:with-impls takes 2 arguments");
  if (!(x instanceof CalcitEnumValue)) throw new Error("&enum:with-impls expects an enum value");
  const impl = coerce_impl(y, "&enum:with-impls");
  let proto = x.enumPrototype;
  if (proto == null) {
    proto = new CalcitEnumDef(new CalcitStructValue(newTag("_"), [], [], new CalcitStructDef(newTag("_"), [], [])));
  }
  return new CalcitEnumValue(x.tag, x.extra, proto.withImpls(impl));
};

export let _$n_enum_$o_impl_traits = function (x: CalcitValue, ...traits: CalcitValue[]) {
  if (traits.length < 1) throw new Error("&enum:impl-traits takes 2+ arguments");
  if (!(x instanceof CalcitEnumValue)) throw new Error("&enum:impl-traits expects an enum value");
  const impls = traits.map((trait) => coerce_impl(trait, "&enum:impl-traits"));
  let proto = x.enumPrototype;
  if (proto == null) {
    const tagName = x.tag instanceof CalcitTag ? x.tag : newTag("tag");
    const anyTypes = new CalcitSliceList(new Array(x.extra.length).fill(newTag("any")));
    proto = new CalcitEnumDef(
      new CalcitStructValue(newTag("_"), [tagName], [anyTypes], new CalcitStructDef(newTag("_"), [tagName], [anyTypes]))
    );
  }
  return new CalcitEnumValue(x.tag, x.extra, proto.withImpls(impls));
};

export let _$n_enum_$o_definition = function (x: CalcitEnumValue) {
  if (arguments.length !== 1) throw new Error("&enum:definition takes 1 argument");
  if (!(x instanceof CalcitEnumValue)) throw new Error("&enum:definition expects an enum value");
  if (x.enumPrototype == null) {
    return null;
  }
  return x.enumPrototype;
};

const unwrap_enum_prototype = (enumPrototype: CalcitValue, procName: string): CalcitStructValue => {
  if (enumPrototype instanceof CalcitEnumDef) {
    return enumPrototype.prototype;
  }
  if (enumPrototype instanceof CalcitStructValue) {
    throw new Error(`${procName} no longer accepts a struct prototype; pass an EnumDef produced by defenum`);
  }
  throw new Error(`${procName} expects an EnumDef as first argument`);
};

const assert_enum_tag_args = (procName: string, enumPrototype: CalcitValue, variantTag: CalcitTag): CalcitStructValue => {
  const proto = unwrap_enum_prototype(enumPrototype, procName);
  if (!(variantTag instanceof CalcitTag)) {
    throw new Error(`${procName} expects tag as second argument`);
  }
  return proto;
};

export let _$n_enum_def_$o_has_variant_$q_ = function (enumPrototype: CalcitValue, variantTag: CalcitTag) {
  if (arguments.length !== 2) throw new Error("&enum-def:has-variant? takes 2 arguments");
  const proto = assert_enum_tag_args("&enum-def:has-variant?", enumPrototype, variantTag);
  return proto.contains(variantTag);
};

export let _$n_enum_def_$o_has_variant = _$n_enum_def_$o_has_variant_$q_;

export let _$n_enum_def_$o_variant_arity = function (enumPrototype: CalcitValue, variantTag: CalcitTag) {
  if (arguments.length !== 2) throw new Error("&enum-def:variant-arity takes 2 arguments");
  const proto = assert_enum_tag_args("&enum-def:variant-arity", enumPrototype, variantTag);

  const variant = proto.getOrNil(variantTag);
  if (variant === undefined) {
    throw new Error(`Variant ${variantTag.value} not found in enum ${proto.name.value}`);
  }

  if (variant instanceof CalcitSliceList) {
    return variant.len();
  }
  throw new Error("Expected variant to be a list");
};

export let _$n_enum_$o_validate = function (enumValue: CalcitValue, tag: CalcitValue): void {
  if (arguments.length !== 2) throw new Error("&enum:validate takes 2 arguments");
  if (!(enumValue instanceof CalcitEnumValue)) throw new Error("&enum:validate expects an enum value as first argument");
  if (enumValue.enumPrototype == null) {
    return;
  }

  const proto = assert_enum_tag_args("&enum:validate", enumValue.enumPrototype as CalcitValue, tag as CalcitTag);

  const tagValue = tag as CalcitTag;
  const variant = proto.getOrNil(tagValue);
  if (variant === undefined) {
    throw new Error(`enum does not have variant ${tagValue.value} for ${enumValue}`);
  }

  if (variant instanceof CalcitSliceList) {
    const expected = variant.len();
    const actual = enumValue.extra.length;
    if (expected !== actual) {
      throw new Error(`enum variant expects ${expected} payload(s), got ${actual} for ${enumValue}`);
    }
    return;
  }

  throw new Error("Expected variant to be a list");
};

export let _$n_struct_$o_get = function (xs: CalcitValue, k: CalcitTag) {
  if (arguments.length !== 2) {
    throw new Error("&struct:get takes 2 arguments");
  }

  if (xs instanceof CalcitStructValue) return xs.getRequired(k);

  throw new Error("Does not support `&get` on this type");
};

export let _$n_list_$o_assoc = function (xs: CalcitValue, k: CalcitValue, v: CalcitValue) {
  if (arguments.length !== 3) throw new Error("assoc takes 3 arguments");

  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    if (typeof k !== "number") {
      throw new Error("Expected number index for lists");
    }
    return xs.assoc(k, v);
  }
  throw new Error("list `assoc` expected a list");
};
export let _$n_enum_$o_assoc = function (xs: CalcitValue, k: CalcitValue, v: CalcitValue) {
  if (arguments.length !== 3) throw new Error("assoc takes 3 arguments");

  if (xs instanceof CalcitEnumValue) {
    if (typeof k !== "number") {
      throw new Error("Expected number index for lists");
    }
    return xs.assoc(k, v);
  }

  throw new Error("enum `assoc` expected an enum value");
};
export let _$n_map_$o_assoc = function (xs: CalcitValue, ...args: CalcitValue[]) {
  if (arguments.length < 3) throw new Error("assoc takes at least 3 arguments");
  if (args.length % 2 !== 0) throw new Error("assoc expected odd arguments");

  if (xs instanceof CalcitMap || xs instanceof CalcitSliceMap) return xs.assoc(...args);

  throw new Error("map `assoc` expected a map");
};
export let _$n_struct_$o_assoc = function (xs: CalcitValue, k: CalcitValue, v: CalcitValue) {
  if (arguments.length !== 3) throw new Error("assoc takes 3 arguments");

  if (xs instanceof CalcitStructValue) return xs.assoc(k, v);

  throw new Error("struct `assoc` expected a struct value");
};

export let _$n_struct_$o_impls = function (xs: CalcitValue) {
  if (arguments.length !== 1) throw new Error("&struct:impls takes 1 argument");
  if (xs instanceof CalcitStructValue) return new CalcitSliceList(xs.structRef.impls);
  throw new Error("&struct:impls expected a struct value");
};

export let _$n_struct_$o_impl_traits = function (xs: CalcitValue, ...traits: CalcitValue[]) {
  if (traits.length < 1) throw new Error("&struct:impl-traits takes 2+ arguments");
  if (!(xs instanceof CalcitStructValue)) throw new Error("&struct:impl-traits expected a struct value");
  const impls = traits.map((trait) => coerce_impl(trait, "&struct:impl-traits"));
  const nextStruct = new CalcitStructDef(xs.name, xs.fields, xs.structRef.fieldTypes, xs.structRef.impls.concat(impls));
  return new CalcitStructValue(xs.name, xs.fields, xs.values, nextStruct);
};

export let _$n_struct_def_$o_impl_traits = function (xs: CalcitValue, ...traits: CalcitValue[]) {
  if (traits.length < 1) throw new Error("&struct-def:impl-traits takes 2+ arguments");
  if (!(xs instanceof CalcitStructDef)) throw new Error("&struct-def:impl-traits expected a struct definition");
  const addedImpls = traits.map((trait) => coerce_impl(trait, "&struct-def:impl-traits"));
  const baseImpls = xs.impls ?? [];
  return new CalcitStructDef(xs.name, xs.fields, xs.fieldTypes, baseImpls.concat(addedImpls));
};

export let _$n_enum_def_$o_impl_traits = function (xs: CalcitValue, ...traits: CalcitValue[]) {
  if (traits.length < 1) throw new Error("&enum-def:impl-traits takes 2+ arguments");
  const addedImpls = traits.map((trait) => coerce_impl(trait, "&enum-def:impl-traits"));
  if (xs instanceof CalcitEnumDef) {
    return xs.withImpls(addedImpls);
  }
  throw new Error("&enum-def:impl-traits expected an enum definition");
};

export let _$n_impl_$o_origin = function (impl: CalcitValue): CalcitValue {
  if (arguments.length !== 1) throw new Error("&impl:origin expected 1 argument");
  if (impl instanceof CalcitImpl) {
    return impl.origin ?? null;
  }
  throw new Error(`&impl:origin expected an impl, but received: ${toString(impl, true)}`);
};

export let _$n_impl_$o_get = function (impl: CalcitValue, name: CalcitValue): CalcitValue {
  if (arguments.length !== 2) throw new Error("&impl:get expected 2 arguments");
  if (!(impl instanceof CalcitImpl)) {
    throw new Error(`&impl:get expected an impl as first argument, but received: ${toString(impl, true)}`);
  }
  return impl.get(name);
};

export let _$n_impl_$o_nth = function (impl: CalcitValue, index: CalcitValue): CalcitValue {
  if (arguments.length !== 2) throw new Error("&impl:nth expected 2 arguments");
  if (!(impl instanceof CalcitImpl)) {
    throw new Error(`&impl:nth expected an impl as first argument, but received: ${toString(impl, true)}`);
  }
  if (typeof index !== "number" || !Number.isInteger(index) || index < 0) {
    throw new Error(`&impl:nth expected a non-negative integer index, but received: ${toString(index, true)}`);
  }
  return impl.values[index];
};

export let _$n_list_$o_assoc_before = function (xs: CalcitList | CalcitSliceList, k: number, v: CalcitValue): CalcitList {
  if (arguments.length !== 3) {
    throw new Error("assoc takes 3 arguments");
  }
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    if (typeof k !== "number") {
      throw new Error("Expected number index for lists");
    }
    return xs.assocBefore(k, v);
  }

  throw new Error("Does not support `assoc-before` on this type");
};

export let _$n_list_$o_assoc_after = function (xs: CalcitSliceList, k: number, v: CalcitValue): CalcitList | CalcitSliceList {
  if (arguments.length !== 3) {
    throw new Error("assoc takes 3 arguments");
  }
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    if (typeof k !== "number") {
      throw new Error("Expected number index for lists");
    }
    return xs.assocAfter(k, v);
  }

  throw new Error("Does not support `assoc-after` on this type");
};

export let _$n_list_$o_dissoc = function (xs: CalcitValue | CalcitSliceList, k: CalcitValue) {
  if (arguments.length !== 2) throw new Error("dissoc takes 2 arguments");

  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    if (typeof k !== "number") throw new Error("Expected number index for lists");

    return xs.dissoc(k);
  }

  throw new Error("`dissoc` expected a list");
};
export let _$n_map_$o_dissoc = function (xs: CalcitValue, ...args: CalcitValue[]) {
  if (args.length < 1) throw new Error("dissoc takes at least 2 arguments");

  if (xs instanceof CalcitMap || xs instanceof CalcitSliceMap) {
    return xs.dissoc(...args);
  }

  throw new Error("`dissoc` expected a map");
};

export let reset_$x_ = (a: CalcitRef, v: CalcitValue): CalcitValue => {
  if (!(a instanceof CalcitRef)) {
    throw new Error("Expected ref for reset!");
  }
  let prev = a.value;
  a.value = v;
  a.listeners.forEach((f) => {
    f(v, prev);
  });
  return v;
};

export let add_watch = (a: CalcitRef, k: CalcitTag, f: CalcitFn): void => {
  if (!(a instanceof CalcitRef)) {
    throw new Error("Expected ref for add-watch!");
  }
  if (!(k instanceof CalcitTag)) {
    throw new Error("Expected watcher key in tag");
  }
  if (!(typeof f === "function")) {
    throw new Error("Expected watcher function");
  }
  a.listeners.set(k, f);
};

export let remove_watch = (a: CalcitRef, k: CalcitTag): void => {
  a.listeners.delete(k);
};

const MAX_RANGE_LENGTH = 0xffff_ffff;

export let range = (n: number, m?: number, step: number = 1): CalcitSliceList => {
  const base = m == null ? 0 : n;
  const bound = m == null ? n : m;

  if (!Number.isFinite(base) || !Number.isFinite(bound) || !Number.isFinite(step)) {
    throw new TypeError("&list:range expected finite numbers for base, bound, and step");
  }
  if (base === bound) {
    return new CalcitSliceList([]);
  }
  if (step === 0 || (bound > base && step < 0) || (bound < base && step > 0)) {
    throw new Error("&list:range cannot construct list with a step of 0 or invalid step direction");
  }

  // Avoid overflowing `bound - base` when finite endpoints have opposite signs.
  const estimatedLength = (base < 0) !== (bound < 0)
    ? Math.ceil(Math.abs(base) / Math.abs(step) + Math.abs(bound) / Math.abs(step))
    : Math.ceil((bound - base) / step);
  if (!Number.isFinite(estimatedLength) || estimatedLength > MAX_RANGE_LENGTH) {
    throw new Error("&list:range result is too large");
  }

  const result: Array<CalcitValue> = [];
  if (step > 0) {
    for (let value = base; value < bound; ) {
      result.push(value);
      const next = value + step;
      if (next === value) {
        throw new Error("&list:range step does not advance the current value");
      }
      value = next;
    }
  } else {
    for (let value = base; value > bound; ) {
      result.push(value);
      const next = value + step;
      if (next === value) {
        throw new Error("&list:range step does not advance the current value");
      }
      value = next;
    }
  }
  return new CalcitSliceList(result);
};

export function _$n_list_$o_empty_$q_(xs: CalcitValue): boolean {
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) return xs.isEmpty();
  throw new Error(`expected a list ${xs}`);
}
export function _$n_str_$o_empty_$q_(xs: CalcitValue): boolean {
  if (typeof xs === "string") return xs.length === 0;
  throw new Error(`expected a string ${xs}`);
}
export function _$n_map_$o_empty_$q_(xs: CalcitValue): boolean {
  if (xs instanceof CalcitMap || xs instanceof CalcitSliceMap) return xs.isEmpty();

  throw new Error(`expected a list ${xs}`);
}
export function _$n_set_$o_empty_$q_(xs: CalcitValue): boolean {
  if (xs instanceof CalcitSet) return xs.len() === 0;
  throw new Error(`expected a list ${xs}`);
}

export let _$n_list_$o_first = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    if (xs.isEmpty()) {
      return null;
    }
    return xs.first();
  }
  console.error(xs);
  throw new Error("Expected a list");
};

export let _$n_list_$o_last = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    if (xs.isEmpty()) {
      return null;
    }
    return xs.get(xs.len() - 1);
  }
  console.error(xs);
  throw new Error("Expected a list");
};

export let _$n_str_$o_first = (xs: CalcitValue): CalcitValue => {
  if (typeof xs === "string") {
    return xs[0];
  }
  console.error(xs);
  throw new Error("Expected a string");
};

export let _$n_map_$o_destruct = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitMap || xs instanceof CalcitSliceMap) {
    // order not stable
    if (xs.len() > 0) {
      let pair = xs.pairs()[0];
      let k0 = pair[0];
      return new CalcitSliceList([pair[0], pair[1], xs.dissoc(k0)]);
    } else {
      return null;
    }
  }
  console.error(xs);
  throw new Error("Expected a map");
};

export let _$n_set_$o_destruct = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitSet) return xs.destruct();

  console.error(xs);
  throw new Error("Expect a set");
};

export let timeout_call = (duration: number, f: CalcitFn): void => {
  if (typeof duration !== "number") {
    throw new Error("Expected duration in number");
  }
  if (typeof f !== "function") {
    throw new Error("Expected callback in fn");
  }
  setTimeout(f, duration);
};

export let _$n_list_$o_rest = (xs: CalcitValue): CalcitList | CalcitSliceList => {
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    if (xs.len() === 0) {
      return xs.slice(0, 0);
    }
    return xs.rest();
  }
  console.error(xs);
  throw new Error("Expected a list");
};

export let _$n_str_$o_rest = (xs: CalcitValue): CalcitValue => {
  if (typeof xs === "string") return xs.slice(1);

  console.error(xs);
  throw new Error("Expects a string");
};

export let recur = (...xs: CalcitValue[]): CalcitRecur => {
  return new CalcitRecur(xs);
};

export let _$n_get_calcit_backend = () => {
  return newTag("js");
};

export let not = (x: boolean): boolean => {
  return !x;
};

export let prepend = (xs: CalcitValue, v: CalcitValue): CalcitList => {
  if (!(xs instanceof CalcitList || xs instanceof CalcitSliceList)) {
    throw new Error("Expected array");
  }
  return xs.prepend(v);
};

export let append = (xs: CalcitValue, v: CalcitValue): CalcitList | CalcitSliceList => {
  if (!(xs instanceof CalcitList || xs instanceof CalcitSliceList)) {
    throw new Error("Expected array");
  }
  return xs.append(v);
};

export let last = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    if (xs.isEmpty()) {
      return null;
    }
    return xs.get(xs.len() - 1);
  }
  if (typeof xs === "string") {
    return xs[xs.length - 1];
  }
  console.error(xs);
  throw new Error("Data not ready for last");
};

export let butlast = (xs: CalcitValue): CalcitList | CalcitSliceList | string => {
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    if (xs.len() === 0) {
      return xs.slice(0, 0);
    }
    return xs.slice(0, xs.len() - 1);
  }
  if (typeof xs === "string") {
    return xs.slice(0, -1);
  }
  console.error(xs);
  throw new Error("Data not ready for butlast");
};

export let initCrTernary = (x: string): CalcitValue => {
  console.error("Ternary for js not implemented yet!");
  return null;
};

export let _SHA__$M_ = (...xs: CalcitValue[]): CalcitValue => {
  var result: CalcitValue[] = [];
  for (let idx = 0; idx < xs.length; idx++) {
    result.push(xs[idx]);
  }
  return new CalcitSet(result);
};

let idCounter = 0;

export let generate_id_$x_ = (): string => {
  // TODO use nanoid.. this code is wrong
  idCounter = idCounter + 1;
  let time = Date.now();
  return `gen_id_${idCounter}_${time}`;
};

export let _$n_display_stack = (): void => {
  console.trace();
};

export let _$n_list_$o_slice = (xs: CalcitList, from: number, to: number): CalcitSliceList | CalcitList => {
  if (xs === null) {
    return null;
  }
  let size = xs.len();
  if (to == null) {
    to = size;
  } else if (to <= from) {
    return new CalcitSliceList([]);
  } else if (to > size) {
    to = size;
  }
  return xs.slice(from, to);
};

export let _$n_list_$o_concat = (...lists: (CalcitList | CalcitSliceList)[]): CalcitList | CalcitSliceList => {
  let result: CalcitSliceList | CalcitList = new CalcitSliceList([]);
  for (let idx = 0; idx < lists.length; idx++) {
    let item = lists[idx];
    if (item === null) {
      continue;
    }
    if (item instanceof CalcitList || item instanceof CalcitSliceList) {
      if (result.isEmpty()) {
        result = item;
      } else {
        result = result.concat(item);
      }
    } else {
      throw new Error("Expected list for concatenation");
    }
  }
  return result;
};

export let _$n_list_$o_reverse = (xs: CalcitList): CalcitList => {
  if (xs === null) {
    return null;
  }
  return xs.reverse();
};

export let format_ternary_tree = (): null => {
  console.warn("No such function for js");
  return null;
};

export let _$n__GT_ = (a: number, b: number): boolean => {
  return a > b;
};
export let _$n__LT_ = (a: number, b: number): boolean => {
  return a < b;
};
export let _$n__ = (a: number, b: number): number => {
  return a - b;
};
export let _$n__SLSH_ = (a: number, b: number): number => {
  return a / b;
};
export let _$n_number_$o_rem = (a: number, b: number): number => {
  return a % b;
};
export let round_$q_ = (a: number) => {
  return a === Math.round(a);
};
export let _$n_str_$o_concat = (a: string, b: string) => {
  // Optimize string concatenation by avoiding unnecessary toString calls
  const aStr = a !== null ? toString(a, false) : "";
  const bStr = b !== null ? toString(b, false) : "";
  return aStr + bStr;
};
export let sort = (xs: CalcitList | CalcitSliceList, f: CalcitFn): CalcitSliceList => {
  if (xs === null) {
    return null;
  }
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    let ys = xs.toArray();
    return new CalcitSliceList(ys.sort(f as any));
  }
  throw new Error("Expected list");
};

export let floor = (n: number): number => {
  return Math.floor(n);
};

export let _$n_merge = (a: CalcitValue, b: CalcitMap | CalcitSliceMap): CalcitValue => {
  if (a === null) {
    return b;
  }
  if (b === null) {
    return a;
  }
  if (a instanceof CalcitMap || a instanceof CalcitSliceMap) {
    if (b instanceof CalcitMap || b instanceof CalcitSliceMap) {
      return a.merge(b);
    } else {
      throw new Error("Expected an argument of map");
    }
  }
  if (a instanceof CalcitStructValue) {
    if (b instanceof CalcitMap || b instanceof CalcitSliceMap) {
      let values = [];
      for (let idx = 0; idx < a.values.length; idx++) {
        values.push(a.values[idx]);
      }
      let pairs = b.pairs();
      for (let idx = 0; idx < pairs.length; idx++) {
        let k = pairs[idx][0];
        let v = pairs[idx][1];
        let field: CalcitTag;
        if (k instanceof CalcitTag) {
          field = k;
        } else {
          field = newTag(getStringName(k));
        }
        let position = a.findIndex(field);
        if (position >= 0) {
          values[position] = v;
        } else {
          throw new Error(`Cannot find field ${field} among (${a.fields.join(", ")})`);
        }
      }
      return new CalcitStructValue(a.name, a.fields, values);
    }
  }
  throw new Error("Expected map or struct value");
};

export let _$n_merge_non_nil = (a: CalcitMap | CalcitSliceMap, b: CalcitMap | CalcitSliceMap): CalcitMap | CalcitSliceMap => {
  if (a === null) {
    return b;
  }
  if (b === null) {
    return a;
  }
  if (!(a instanceof CalcitMap || a instanceof CalcitSliceMap)) {
    throw new Error("Expected map");
  }
  if (!(b instanceof CalcitMap || b instanceof CalcitSliceMap)) {
    throw new Error("Expected map");
  }

  return a.mergeSkip(b, null);
};

export let to_pairs = (xs: CalcitValue): CalcitValue | CalcitSliceList => {
  if (xs instanceof CalcitMap || xs instanceof CalcitSliceMap) {
    let result: Array<CalcitSliceList> = [];
    let pairs = xs.pairs();
    for (let idx = 0; idx < pairs.length; idx++) {
      result.push(new CalcitSliceList(pairs[idx]));
    }
    return new CalcitSet(result);
  } else if (xs instanceof CalcitStructValue) {
    let arr_result: Array<CalcitSliceList> = [];
    for (let idx = 0; idx < xs.fields.length; idx++) {
      arr_result.push(new CalcitSliceList([xs.fields[idx], xs.values[idx]]));
    }
    return new CalcitSet(arr_result);
  } else {
    throw new Error("Expected a map");
  }
};

// Math functions

export let sin = (n: number) => {
  return Math.sin(n);
};
export let cos = (n: number) => {
  return Math.cos(n);
};
export let pow = (n: number, m: number) => {
  return Math.pow(n, m);
};
export let ceil = (n: number) => {
  return Math.ceil(n);
};
export let round = (n: number) => {
  return Math.round(n);
};
export let _$n_number_$o_fract = (n: number) => {
  return n - Math.floor(n);
};
export let sqrt = (n: number) => {
  return Math.sqrt(n);
};

// Set functions

export let _$n_include = (xs: CalcitSet, y: CalcitValue): CalcitSet => {
  if (!(xs instanceof CalcitSet)) {
    throw new Error("Expected a set");
  }
  if (y === null) {
    return xs;
  }
  return xs.include(y);
};

export let _$n_exclude = (xs: CalcitSet, y: CalcitValue): CalcitSet => {
  if (!(xs instanceof CalcitSet)) {
    throw new Error("Expected a set");
  }
  if (y === null) {
    return xs;
  }
  return xs.exclude(y);
};

export let _$n_difference = (xs: CalcitSet, ys: CalcitSet): CalcitSet => {
  if (!(xs instanceof CalcitSet)) {
    throw new Error("Expected a set");
  }
  if (!(ys instanceof CalcitSet)) {
    throw new Error("Expected a set for ys");
  }
  return xs.difference(ys);
};

export let _$n_union = (xs: CalcitSet, ys: CalcitSet): CalcitSet => {
  if (!(xs instanceof CalcitSet)) {
    throw new Error("Expected a set");
  }
  if (!(ys instanceof CalcitSet)) {
    throw new Error("Expected a set for ys");
  }
  return xs.union(ys);
};

export let _$n_set_$o_intersection = (xs: CalcitSet, ys: CalcitSet): CalcitSet => {
  if (!(xs instanceof CalcitSet)) {
    throw new Error("Expected a set");
  }
  if (!(ys instanceof CalcitSet)) {
    throw new Error("Expected a set for ys");
  }
  return xs.intersection(ys);
};

export let _$n_str_$o_replace = (x: string, y: string, z: string): string => {
  // Use a callback replacement so `$` sequences in `z` remain literal, matching Rust's
  // `str::replace`. Escaping `y` preserves the proc's literal-pattern contract, while the
  // global regex completes each original match once even when `z` contains `y`.
  const literalPattern = y.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return x.replace(new RegExp(literalPattern, "gu"), () => z);
};

export let split = (xs: string, x: string): CalcitSliceList => {
  return new CalcitSliceList(xs.split(x));
};
export let split_lines = (xs: string): CalcitSliceList => {
  return new CalcitSliceList(xs.split("\n"));
};
export let _$n_str_$o_slice = (xs: string, m: number, n: number): string => {
  if (n <= m) {
    console.warn("endIndex too small");
    return "";
  }
  return xs.substring(m, n);
};

export let _$n_str_$o_find_index = (x: string, y: string): number => {
  return x.indexOf(y);
};

export let _$n_parse_float = (x: string): number | null => {
  if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/.test(x)) {
    return null;
  }
  const value = Number(x);
  return Number.isNaN(value) ? null : value;
};
export let trim = (x: string, c: string): string => {
  if (c != null) {
    if (c.length !== 1) {
      throw new Error("Expected c of a character");
    }
    var buffer = x;
    var size = buffer.length;
    var idx = 0;
    while (idx < size && buffer[idx] === c) {
      idx = idx + 1;
    }
    buffer = buffer.substring(idx);
    var size = buffer.length;
    var idx = size;
    while (idx > 1 && buffer[idx - 1] === c) {
      idx = idx - 1;
    }
    buffer = buffer.substring(0, idx);
    return buffer;
  }
  return x.trim();
};

export let _$n_number_$o_format = (x: number, n: number): string => {
  return x.toFixed(n);
};

export let _$n_number_$o_display_by = (x: number, n: number): string => {
  switch (n) {
    case 2:
      return `0b${x.toString(2)}`;
    case 8:
      return `0o${x.toString(8)}`;
    case 16:
      return `0x${x.toString(16)}`;
    default:
      throw new Error("Expected n of 2, 8, or 16");
  }
};

export let get_char_code = (c: string): number => {
  if (typeof c !== "string" || c.length !== 1) {
    throw new Error("Expected a character");
  }
  return c.charCodeAt(0);
};

export let char_from_code = (n: number): string => {
  if (typeof n !== "number") throw new Error("Expected an integer");
  return String.fromCharCode(n);
};

export let _$n_set_$o_to_list = (x: CalcitSet): CalcitSliceList => {
  return new CalcitSliceList(x.values());
};

export let aget = (x: any, name: string): any => {
  return x[name];
};
export let aset = (x: any, name: string, v: any): any => {
  return (x[name] = v);
};
export let js_get = (x: any, name: string): any => x[name] ?? null;
export let js_set = aset;
/** generates `delete a.b` */
export let js_delete = (obj: any, name: string): any => {
  return delete obj[name];
};

export let _$n_get_env = (name: string, v0?: CalcitValue): CalcitValue => {
  let v = undefined;
  if (inNodeJs) {
    // only available for Node.js
    v = process.env[name];
  } else if (typeof URLSearchParams != null && typeof location != null) {
    v = new URLSearchParams(location.search).get(name);
  }
  if (v != null && v0 != null) {
    console.log(`(get-env ${name}): ${v}`);
  }
  if (v == null && v0 == null) {
    console.warn(`(get-env "${name}"): config not found`);
  }
  return v ?? v0 ?? null;
};

// Keep already-emitted JS bundles loadable across the nominal Option migration.
// Newly generated code calls the internal raw proc; source-level `get-env`
// remains the typed Option wrapper defined in calcit.core.
export let get_env = _$n_get_env;

export let turn_tag = (x: CalcitValue): CalcitTag => {
  if (typeof x === "string") {
    return newTag(x);
  }
  if (x instanceof CalcitTag) {
    return x;
  }
  if (x instanceof CalcitSymbol) {
    return newTag(x.value);
  }
  console.error(x);
  throw new Error("Unexpected data for tag");
};

export let turn_symbol = (x: CalcitValue): CalcitSymbol => {
  if (typeof x === "string") {
    return new CalcitSymbol(x);
  }
  if (x instanceof CalcitSymbol) {
    return x;
  }
  if (x instanceof CalcitTag) {
    return new CalcitSymbol(x.value);
  }
  console.error(x);
  throw new Error("Unexpected data for symbol");
};

export let to_lispy_string = (...args: CalcitValue[]): string => {
  return args.map((x) => toString(x, true)).join(" ");
};

/** helper function for println, js only */
export let printable = (...args: CalcitValue[]): string => {
  return args.map((x) => toString(x, false)).join(" ");
};

// time from app start
export let cpu_time = (): number => {
  if (inNodeJs) {
    // uptime returns in seconds
    return process.uptime() * 1000;
  }
  // returns in milliseconds
  return performance.now();
};

export let quit_$x_ = (): void => {
  if (inNodeJs) {
    process.exit(1);
  } else {
    throw new Error("quit!()");
  }
};

export let turn_string = (x: CalcitValue): string => {
  if (x === null) {
    return "";
  }
  if (typeof x === "string") {
    return x;
  }
  if (x instanceof CalcitTag) {
    return x.value;
  }
  if (x instanceof CalcitSymbol) {
    return x.value;
  }
  if (typeof x === "number") {
    return x.toString();
  }
  if (typeof x === "boolean") {
    return x.toString();
  }
  console.error(x);
  throw new Error("Unexpected data to turn string");
};

export let identical_$q_ = (x: CalcitValue, y: CalcitValue): boolean => {
  return x === y;
};

export let starts_with_$q_ = (xs: CalcitValue, y: CalcitValue): boolean => {
  if (typeof xs === "string" && typeof y === "string") {
    return xs.startsWith(y);
  }
  if (xs instanceof CalcitTag && y instanceof CalcitTag) {
    return xs.value.startsWith(y.value);
  }
  if (xs instanceof CalcitTag && typeof y === "string") {
    return xs.value.startsWith(y);
  }
  throw new Error("expected strings or tags");
};
export let ends_with_$q_ = (xs: string, y: string): boolean => {
  return xs.endsWith(y);
};

export let blank_$q_ = (x: string): boolean => {
  if (x === null) {
    return true;
  }
  if (typeof x === "string") {
    return x.trim() === "";
  } else {
    throw new Error("Expected a string");
  }
};

export let _$n_str_$o_compare = (x: string, y: string) => {
  if (x < y) {
    return -1;
  }
  if (x > y) {
    return 1;
  }
  return 0;
};

export let arrayToList = (xs: Array<CalcitValue>): CalcitSliceList => {
  return new CalcitSliceList(xs ?? []);
};

export let listToArray = (xs: CalcitList | CalcitSliceList): Array<CalcitValue> => {
  if (xs === null) {
    return null;
  }
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    return xs.toArray();
  } else {
    throw new Error("Expected list");
  }
};

export let number_$q_ = (x: CalcitValue): boolean => {
  return typeof x === "number";
};
export let string_$q_ = (x: CalcitValue): boolean => {
  return typeof x === "string";
};
export let bool_$q_ = (x: CalcitValue): boolean => {
  return typeof x === "boolean";
};
export let nil_$q_ = (x: CalcitValue): boolean => {
  return x === null;
};
export let tag_$q_ = (x: CalcitValue): boolean => {
  return x instanceof CalcitTag;
};
export let symbol_$q_ = (x: CalcitValue): boolean => {
  return x instanceof CalcitSymbol;
};
export let map_$q_ = (x: CalcitValue): boolean => {
  return x instanceof CalcitSliceMap || x instanceof CalcitMap;
};
export let list_$q_ = (x: CalcitValue): boolean => {
  return x instanceof CalcitSliceList || x instanceof CalcitList;
};
export let set_$q_ = (x: CalcitValue): boolean => {
  return x instanceof CalcitSet;
};
export let fn_$q_ = (x: CalcitValue): boolean => {
  return typeof x === "function";
};
export let ref_$q_ = (x: CalcitValue): boolean => {
  return x instanceof CalcitRef;
};
export let struct_$q_ = (x: CalcitValue): boolean => {
  if (x instanceof CalcitStructDef) {
    throw new Error("`struct?` now checks struct values; use `struct-def?` for a value produced by `defstruct`");
  }
  return x instanceof CalcitStructValue;
};
export let enum_$q_ = (x: CalcitValue): boolean => {
  if (x instanceof CalcitEnumDef) {
    throw new Error("`enum?` now checks enum values; use `enum-def?` for a value produced by `defenum`");
  }
  return x instanceof CalcitEnumValue;
};
export let buffer_$q_ = (x: CalcitValue): boolean => {
  console.warn("TODO, detecting buffer");
  return false;
};

export let _$n_str_$o_escape = (x: string) => JSON.stringify(x);

export type CalcitFileInjections = {
  read_file?: (path: string) => unknown;
  read_dir?: (path: string, recursive: boolean) => unknown;
  write_file?: (path: string, content: string) => unknown;
};

const get_file_injection = <K extends keyof CalcitFileInjections>(name: K): NonNullable<CalcitFileInjections[K]> => {
  const injections = (globalThis as { __calcit_injections__?: CalcitFileInjections }).__calcit_injections__;
  const injection = injections?.[name];
  if (typeof injection !== "function") {
    throw new Error(`${name} is unavailable: the JavaScript host did not provide a __calcit_injections__.${name} function`);
  }
  return injection as NonNullable<CalcitFileInjections[K]>;
};

export let read_file = (path: string): string => {
  if (inNodeJs) {
    const content = get_file_injection("read_file")(path);
    if (typeof content !== "string") {
      throw new TypeError(`read_file injection expected a string result, got ${typeof content}`);
    }
    return content;
  } else {
    // no actual File API in browser
    const content = localStorage.getItem(path);
    if (content === null) {
      throw new Error(`read-file failed at ${path}: key not found in localStorage`);
    }
    return content;
  }
};
export let read_dir = (path: string, recursive: boolean): CalcitSliceList => {
  if (!inNodeJs) {
    throw new Error("read_dir is unavailable in browser JavaScript hosts");
  }
  const paths = get_file_injection("read_dir")(path, recursive);
  if (!Array.isArray(paths) || !paths.every((item) => typeof item === "string")) {
    throw new TypeError("read_dir injection expected an array of strings");
  }
  return new CalcitSliceList([...paths].sort());
};
export let write_file = (path: string, content: string): void => {
  if (inNodeJs) {
    get_file_injection("write_file")(path, content);
  } else {
    // no actual File API in browser
    localStorage.setItem(path, content);
  }
};

export let parse_cirru = (code: string): CalcitCirruQuote => {
  return new CalcitCirruQuote(parse(code));
};

// for JavaScript, it's same as parse_cirru
export let parse_cirru_list = (code: string): CalcitList => {
  return to_calcit_data(parse(code), true) as CalcitList;
};

export let parse_cirru_edn = (code: string, options: CalcitValue) => {
  let nodes = parse(code);
  if (nodes.length === 1) {
    return extract_cirru_edn(nodes[0], options);
  } else {
    throw new Error(`Expected EDN in a single node, got ${nodes.length}`);
  }
};

type DataShapeNode =
  | { kind: "nil" | "unit" | "bool" | "number" | "string" | "symbol" | "tag" | "buffer" | "cirru-quote" | "dynamic" }
  | { kind: "optional" | "list" | "set" | "ref"; inner: number }
  | { kind: "map"; key: number; value: number }
  | { kind: "map-option"; nominal: CalcitEnumDef; inner: number }
  | { kind: "struct"; nominal: CalcitStructDef; fields: Array<[string, number]> }
  | { kind: "enum"; nominal: CalcitEnumDef; variants: Array<{ tag: string; payload: number[] }> };

type DataShapeGraph = {
  version: number;
  root: number;
  fingerprint: string;
  nodes: DataShapeNode[];
};

const typed_edn_kind = (value: any): string => {
  if (value === null) return "nil";
  if (value === undefined) return "unit";
  if (typeof value === "boolean") return "bool";
  if (typeof value === "number") return "number";
  if (typeof value === "string") return "string";
  if (value instanceof CalcitSymbol) return "symbol";
  if (value instanceof CalcitTag) return "tag";
  if (value instanceof CalcitList || value instanceof CalcitSliceList) return "list";
  if (value instanceof CalcitMap || value instanceof CalcitSliceMap) return "map";
  if (value instanceof CalcitSet || value instanceof TypedEdnSetView) return "set";
  if (value instanceof CalcitStructValue) return "struct";
  if (value instanceof CalcitEnumValue) return "enum";
  if (value instanceof CalcitRef) return "atom";
  if (value instanceof CalcitCirruQuote) return "cirru-quote";
  if (value instanceof Uint8Array) return "buffer";
  return "unsupported";
};

const typed_edn_error = (path: string, message: string): never => {
  throw new Error(`parse-cirru-edn-as failed at ${path}: ${message}`);
};

const enum_prototype_name = (value: CalcitEnumDef | CalcitStructValue): string => {
  return value instanceof CalcitEnumDef ? value.name() : value.name.value;
};

const decode_typed_edn_node = (graph: DataShapeGraph, nodeId: number, input: any, path: string, depth: number): any => {
  if (depth > 1024) typed_edn_error(path, "decode nesting exceeds 1024");
  const node = graph.nodes[nodeId];
  if (node == null) typed_edn_error(path, `invalid data shape node #${nodeId}`);

  switch (node.kind) {
    case "nil":
      if (input === null) return null;
      return typed_edn_error(path, `expected Nil, got ${typed_edn_kind(input)}`);
    case "unit":
      if (input === undefined) return input;
      return typed_edn_error(path, `expected Unit, got ${typed_edn_kind(input)}`);
    case "bool":
      if (typeof input === "boolean") return input;
      return typed_edn_error(path, `expected bool, got ${typed_edn_kind(input)}`);
    case "number":
      if (typeof input === "number") return input;
      return typed_edn_error(path, `expected number, got ${typed_edn_kind(input)}`);
    case "string":
      if (typeof input === "string") return input;
      return typed_edn_error(path, `expected string, got ${typed_edn_kind(input)}`);
    case "symbol":
      if (input instanceof CalcitSymbol) return input;
      return typed_edn_error(path, `expected symbol, got ${typed_edn_kind(input)}`);
    case "tag":
      if (input instanceof CalcitTag) return input;
      return typed_edn_error(path, `expected tag, got ${typed_edn_kind(input)}`);
    case "buffer":
      if (input instanceof Uint8Array) return input;
      return typed_edn_error(path, `expected buffer, got ${typed_edn_kind(input)}`);
    case "cirru-quote":
      if (input instanceof CalcitCirruQuote) return input;
      return typed_edn_error(path, `expected cirru-quote, got ${typed_edn_kind(input)}`);
    case "optional":
      return input === null ? null : decode_typed_edn_node(graph, node.inner, input, path, depth + 1);
    case "list": {
      if (!(input instanceof CalcitList || input instanceof CalcitSliceList)) {
        return typed_edn_error(path, `expected list, got ${typed_edn_kind(input)}`);
      }
      const values = Array.from(input.items()).map((value, idx) =>
        decode_typed_edn_node(graph, node.inner, value, `${path}[${idx}]`, depth + 1)
      );
      return new CalcitSliceList(values);
    }
    case "set": {
      if (!(input instanceof CalcitSet || input instanceof TypedEdnSetView)) {
        return typed_edn_error(path, `expected set, got ${typed_edn_kind(input)}`);
      }
      const values: any[] = [];
      const inputValues = input instanceof TypedEdnSetView ? input.items : input.values();
      inputValues.forEach((value) => {
        const itemPath = `${path}.item`;
        const decoded = decode_typed_edn_node(graph, node.inner, value, itemPath, depth + 1);
        if (values.some((existing) => _$n__$e_(existing, decoded))) {
          typed_edn_error(itemPath, "duplicate decoded set value");
        }
        values.push(decoded);
      });
      return new CalcitSet(values);
    }
    case "map": {
      if (!(input instanceof CalcitMap || input instanceof CalcitSliceMap)) {
        return typed_edn_error(path, `expected map, got ${typed_edn_kind(input)}`);
      }
      const entries: any[] = [];
      input.pairs().forEach(([key, value]) => {
        const keyPath = `${path}.key`;
        const decodedKey = decode_typed_edn_node(graph, node.key, key, keyPath, depth + 1);
        for (let idx = 0; idx < entries.length; idx += 2) {
          if (_$n__$e_(entries[idx], decodedKey)) {
            typed_edn_error(keyPath, "duplicate decoded map key");
          }
        }
        const decodedValue = decode_typed_edn_node(graph, node.value, value, `${path}.value`, depth + 1);
        entries.push(decodedKey, decodedValue);
      });
      return new CalcitSliceMap(entries);
    }
    case "ref":
      if (!(input instanceof CalcitRef)) return typed_edn_error(path, `expected atom, got ${typed_edn_kind(input)}`);
      return atom(decode_typed_edn_node(graph, node.inner, input.value, `${path}.value`, depth + 1));
    case "struct": {
      if (!(input instanceof CalcitStructValue)) {
        return typed_edn_error(path, `expected struct :${node.nominal.name.value}, got ${typed_edn_kind(input)}`);
      }
      if (input.name.value !== node.nominal.name.value) {
        return typed_edn_error(path, `expected struct :${node.nominal.name.value}, got struct :${input.name.value}`);
      }
      const expectedNames = node.fields.map(([name]) => name);
      const actualNames = input.fields.map((field) => field.value);
      const missing = expectedNames.filter((name) => !actualNames.includes(name)).sort();
      const unknown = actualNames.filter((name) => !expectedNames.includes(name)).sort();
      if (missing.length > 0 || unknown.length > 0 || expectedNames.length !== actualNames.length) {
        return typed_edn_error(
          path,
          `struct :${node.nominal.name.value} fields mismatch; missing [${missing.join(", ")}], unknown [${unknown.join(", ")}]`
        );
      }
      const decodedFields = new Map<string, CalcitValue>();
      node.fields.forEach(([name, fieldNode]) => {
        const idx = actualNames.indexOf(name);
        decodedFields.set(name, decode_typed_edn_node(graph, fieldNode, input.values[idx], `${path}.${name}`, depth + 1));
      });
      // Re-align by name so values produced by older runtimes or external data
      // still adopt the current nominal declaration's canonical field layout.
      if (decodedFields.size !== node.nominal.fields.length) {
        return typed_edn_error(path, `struct :${node.nominal.name.value} decoder fields do not match its nominal declaration`);
      }
      const values = node.nominal.fields.map((field) => {
        if (!decodedFields.has(field.value)) {
          return typed_edn_error(path, `struct :${node.nominal.name.value} is missing declared field :${field.value}`);
        }
        return decodedFields.get(field.value)!;
      });
      return new CalcitStructValue(node.nominal.name, node.nominal.fields, values, node.nominal);
    }
    case "enum": {
      if (!(input instanceof CalcitEnumValue) || input.enumPrototype == null) {
        return typed_edn_error(path, `expected enum :${node.nominal.name()}, got ${typed_edn_kind(input)}`);
      }
      const actualEnumName = enum_prototype_name(input.enumPrototype);
      if (actualEnumName !== node.nominal.name()) {
        return typed_edn_error(path, `expected enum :${node.nominal.name()}, got enum :${actualEnumName}`);
      }
      if (!(input.tag instanceof CalcitTag)) {
        return typed_edn_error(path, `enum :${node.nominal.name()} variant must be a tag`);
      }
      const inputTag = input.tag;
      const variant = node.variants.find((candidate) => candidate.tag === inputTag.value);
      if (variant == null) {
        return typed_edn_error(path, `enum :${node.nominal.name()} has no variant :${inputTag.value}`);
      }
      if (variant.payload.length !== input.extra.length) {
        return typed_edn_error(
          path,
          `enum :${node.nominal.name()} variant :${variant.tag} expects ${variant.payload.length} payload(s), got ${input.extra.length}`
        );
      }
      const values = variant.payload.map((payloadNode, idx) =>
        decode_typed_edn_node(graph, payloadNode, input.extra[idx], `${path}.payload[${idx}]`, depth + 1)
      );
      return new CalcitEnumValue(newTag(variant.tag), values, node.nominal);
    }
    default:
      return typed_edn_error(path, `invalid data shape node kind: ${(node as any).kind}`);
  }
};

export let parse_cirru_edn_as = (code: string, graph: DataShapeGraph): CalcitValue => {
  if (typeof code !== "string") throw new Error(`parse-cirru-edn-as expected a string, got ${typed_edn_kind(code)}`);
  if (graph.version !== 2) throw new Error(`parse-cirru-edn-as expected data shape ABI version 2, got ${graph.version}`);
  if (typeof graph.fingerprint !== "string" || graph.fingerprint.length === 0) {
    throw new Error("parse-cirru-edn-as expected a non-empty data shape fingerprint");
  }
  const enumOptions: CalcitValue[] = [];
  for (const node of graph.nodes) {
    if (node.kind === "enum") enumOptions.push(newTag(node.nominal.name()), node.nominal);
  }

  let input: any;
  try {
    const nodes = parse(code);
    if (nodes.length !== 1) throw new Error(`expected EDN in a single node, got ${nodes.length}`);
    input = extract_cirru_edn_for_typed(nodes[0], new CalcitSliceMap(enumOptions));
  } catch (error) {
    const message = error instanceof Error ? error.message : `${error}`;
    throw new Error(`parse-cirru-edn-as failed to parse Cirru EDN: ${message}`);
  }
  return decode_typed_edn_node(graph, graph.root, input, "$", 0) as CalcitValue;
};

const map_decode_error = (path: string, message: string): never => {
  throw new Error(`decode-map-as failed at ${path}: ${message}`);
};

const decode_runtime_map_node = (graph: DataShapeGraph, nodeId: number, input: any, path: string, depth: number): any => {
  if (depth > 1024) map_decode_error(path, "decode nesting exceeds 1024");
  const node = graph.nodes[nodeId];
  if (node == null) map_decode_error(path, `invalid data shape node #${nodeId}`);

  switch (node.kind) {
    case "dynamic":
      return input;
    case "map-option": {
      if (
        input instanceof CalcitEnumValue &&
        (input.enumPrototype === node.nominal || input.enumPrototype?.name() === node.nominal.name())
      ) {
        const tag: CalcitValue = input.tag;
        if (!(tag instanceof CalcitTag)) map_decode_error(path, "Option variant is not a tag");
        const tag_name = (tag as CalcitTag).value;
        if (tag_name === "none" && input.extra.length === 0) return input;
        if (tag_name === "some" && input.extra.length === 1) {
          return new CalcitEnumValue(tag as CalcitTag, [decode_runtime_map_node(graph, node.inner, input.extra[0], path, depth + 1)], node.nominal);
        }
        return map_decode_error(path, "invalid Option value");
      }
      return new CalcitEnumValue(newTag("some"), [decode_runtime_map_node(graph, node.inner, input, path, depth + 1)], node.nominal);
    }
    case "struct": {
      if (!(input instanceof CalcitMap || input instanceof CalcitSliceMap)) {
        return map_decode_error(path, `expected map for struct :${node.nominal.name.value}, got ${typed_edn_kind(input)}`);
      }
      const entries = new Map<string, any>();
      input.pairs().forEach(([key, value]) => {
        const name = key instanceof CalcitTag ? key.value : typeof key === "string" ? key : null;
        if (name == null) map_decode_error(path, "struct map keys must be tags or strings");
        if (!node.fields.some(([field]) => field === name)) {
          map_decode_error(path, `struct :${node.nominal.name.value} has unknown field :${name}`);
        }
        if (entries.has(name)) map_decode_error(path, `struct :${node.nominal.name.value} has duplicate field :${name}`);
        entries.set(name, value);
      });
      const decoded = new Map<string, any>();
      node.fields.forEach(([field, child]) => {
        if (entries.has(field)) {
          decoded.set(field, decode_runtime_map_node(graph, child, entries.get(field), `${path}.${field}`, depth + 1));
        } else {
          const fieldNode = graph.nodes[child];
          if (fieldNode?.kind === "map-option") {
            decoded.set(field, new CalcitEnumValue(newTag("none"), [], fieldNode.nominal));
          } else {
            map_decode_error(path, `struct :${node.nominal.name.value} is missing required field :${field}`);
          }
        }
      });
      const values = node.nominal.fields.map((field) => decoded.get(field.value));
      return new CalcitStructValue(node.nominal.name, node.nominal.fields, values, node.nominal);
    }
    case "list": {
      if (!(input instanceof CalcitList || input instanceof CalcitSliceList)) {
        return map_decode_error(path, `expected list, got ${typed_edn_kind(input)}`);
      }
      return new CalcitSliceList(Array.from(input.items()).map((value, idx) => decode_runtime_map_node(graph, node.inner, value, `${path}[${idx}]`, depth + 1)));
    }
    case "set": {
      if (!(input instanceof CalcitSet || input instanceof TypedEdnSetView)) {
        return map_decode_error(path, `expected set, got ${typed_edn_kind(input)}`);
      }
      const values: any[] = [];
      const source = input instanceof TypedEdnSetView ? input.items : input.values();
      source.forEach((value) => {
        const decoded = decode_runtime_map_node(graph, node.inner, value, `${path}.item`, depth + 1);
        if (values.some((item) => _$n__$e_(item, decoded))) map_decode_error(path, "duplicate decoded set value");
        values.push(decoded);
      });
      return new CalcitSet(values);
    }
    case "map": {
      if (!(input instanceof CalcitMap || input instanceof CalcitSliceMap)) {
        return map_decode_error(path, `expected map, got ${typed_edn_kind(input)}`);
      }
      const entries: any[] = [];
      input.pairs().forEach(([key, value]) => {
        const decodedKey = decode_runtime_map_node(graph, node.key, key, `${path}.key`, depth + 1);
        if (entries.some(([existing]) => _$n__$e_(existing, decodedKey))) map_decode_error(path, "duplicate decoded map key");
        entries.push([decodedKey, decode_runtime_map_node(graph, node.value, value, `${path}.value`, depth + 1)]);
      });
      return new CalcitSliceMap(entries.flat());
    }
    case "optional":
      return input === null ? null : decode_runtime_map_node(graph, node.inner, input, path, depth + 1);
    case "ref":
      if (!(input instanceof CalcitRef)) return map_decode_error(path, `expected atom, got ${typed_edn_kind(input)}`);
      return atom(decode_runtime_map_node(graph, node.inner, input.value, `${path}.value`, depth + 1));
    case "enum": {
      if (!(input instanceof CalcitEnumValue) || input.enumPrototype == null) {
        return map_decode_error(path, `expected enum :${node.nominal.name()}, got ${typed_edn_kind(input)}`);
      }
      const actualEnumName = enum_prototype_name(input.enumPrototype);
      if (actualEnumName !== node.nominal.name()) {
        return map_decode_error(path, `expected enum :${node.nominal.name()}, got enum :${actualEnumName}`);
      }
      if (!(input.tag instanceof CalcitTag)) return map_decode_error(path, `enum :${node.nominal.name()} variant must be a tag`);
      const inputTag = input.tag as CalcitTag;
      const variant = node.variants.find((candidate) => candidate.tag === inputTag.value);
      if (variant == null) return map_decode_error(path, `enum :${node.nominal.name()} has no variant :${inputTag.value}`);
      if (variant.payload.length !== input.extra.length) {
        return map_decode_error(
          path,
          `enum :${node.nominal.name()} variant :${variant.tag} expects ${variant.payload.length} payload(s), got ${input.extra.length}`
        );
      }
      const values = variant.payload.map((payloadNode, idx) =>
        decode_runtime_map_node(graph, payloadNode, input.extra[idx], `${path}.payload[${idx}]`, depth + 1)
      );
      return new CalcitEnumValue(newTag(variant.tag), values, node.nominal);
    }
    default:
      return decode_typed_edn_node(graph, nodeId, input, path, depth);
  }
};

export let decode_map_as = (value: CalcitValue, graph: DataShapeGraph): CalcitValue => {
  if (graph.version !== 2) throw new Error(`decode-map-as expected data shape ABI version 2, got ${graph.version}`);
  if (typeof graph.fingerprint !== "string" || graph.fingerprint.length === 0) {
    throw new Error("decode-map-as expected a non-empty data shape fingerprint");
  }
  return decode_runtime_map_node(graph, graph.root, value, "$", 0) as CalcitValue;
};

const json_to_calcit = (value: any): CalcitValue => {
  if (value == null) return null;
  if (typeof value === "string") return value;
  if (typeof value === "number") return value;
  if (typeof value === "boolean") return value;
  if (Array.isArray(value)) {
    return new CalcitSliceList(value.map(json_to_calcit));
  }
  if (typeof value === "object") {
    const entries: CalcitValue[] = [];
    for (const key of Object.keys(value)) {
      entries.push(newTag(key), json_to_calcit(value[key]));
    }
    return new CalcitSliceMap(entries);
  }
  throw new Error(`Unsupported JSON value: ${value}`);
};

const calcit_json_key = (value: CalcitValue): string => {
  if (value instanceof CalcitTag) return value.value;
  if (typeof value === "string") return value;
  throw new Error(`json-stringify expected object keys to be tags or strings, got: ${toString(value, true)}`);
};

const cirru_quote_to_json = (value: ICirruNode): any => {
  if (typeof value === "string") return value;
  if (Array.isArray(value)) return value.map(cirru_quote_to_json);
  throw new Error(`Unsupported cirru quote node: ${value}`);
};

const calcit_to_json = (value: CalcitValue): any => {
  if (value === null) return null;
  if (typeof value === "string") return value;
  if (typeof value === "number") {
    if (Number.isFinite(value)) return value;
    throw new Error(`json-stringify cannot encode number: ${value}`);
  }
  if (typeof value === "boolean") return value;
  if (value instanceof CalcitTag) return value.value;
  if (value instanceof CalcitSymbol) return value.value;
  if (value instanceof CalcitList || value instanceof CalcitSliceList) {
    return Array.from(value.items()).map(calcit_to_json);
  }
  if (value instanceof CalcitSet) {
    return value.values().map(calcit_to_json);
  }
  if (value instanceof CalcitMap || value instanceof CalcitSliceMap) {
    const result: Record<string, any> = {};
    for (const [key, item] of value.pairs()) {
      result[calcit_json_key(key)] = calcit_to_json(item);
    }
    return result;
  }
  if (value instanceof CalcitEnumValue) {
    return [calcit_to_json(value.tag), ...value.extra.map(calcit_to_json)];
  }
  if (value instanceof CalcitCirruQuote) {
    return cirru_quote_to_json(value.value as ICirruNode);
  }
  if (value instanceof CalcitStructValue) {
    const result: Record<string, any> = {};
    for (let idx = 0; idx < value.fields.length; idx++) {
      result[value.fields[idx].value] = calcit_to_json(value.values[idx]);
    }
    return result;
  }
  throw new Error(`json-stringify cannot encode value: ${toString(value, true)}`);
};

export let json_parse = function (code: CalcitValue): CalcitValue {
  if (arguments.length !== 1) throw new Error("json-parse expected 1 argument");
  if (typeof code !== "string") {
    throw new Error(`json-parse expected a string, got: ${toString(code, true)}`);
  }
  return json_to_calcit(JSON.parse(code));
};

export let json_stringify = function (value: CalcitValue): string {
  if (arguments.length !== 1) throw new Error("json-stringify expected 1 argument");
  return JSON.stringify(calcit_to_json(value));
};

export let json_pretty = function (value: CalcitValue): string {
  if (arguments.length !== 1) throw new Error("json-pretty expected 1 argument");
  return JSON.stringify(calcit_to_json(value), null, 2);
};

export let format_to_lisp = (x: CalcitValue): string => {
  if (x === null) {
    return "nil";
  } else if (x === undefined) {
    return "&unit";
  } else if (x instanceof CalcitSymbol) {
    return x.value;
  } else if (x instanceof CalcitList || x instanceof CalcitSliceList) {
    let chunk = "(";
    for (let item of x.items()) {
      if (chunk != "(") {
        chunk += " ";
      }
      chunk += format_to_lisp(item);
    }
    chunk += ")";
    return chunk;
  } else if (typeof x === "string") {
    return JSON.stringify("|" + x);
  } else {
    return x.toString();
  }
};

export let format_to_cirru = (x: CalcitValue): string => {
  let xs = transform_code_to_cirru(x);
  console.log("tree", xs);
  return writeCirruCode([xs], { useInline: false });
};

export let transform_code_to_cirru = (x: CalcitValue): ICirruNode => {
  if (x === null) {
    return "nil";
  } else if (x === undefined) {
    return "&unit";
  } else if (x instanceof CalcitSymbol) {
    return x.value;
  } else if (x instanceof CalcitList || x instanceof CalcitSliceList) {
    let xs: ICirruNode[] = [];
    for (let item of x.items()) {
      xs.push(transform_code_to_cirru(item));
    }
    return xs;
  } else if (typeof x === "string") {
    return JSON.stringify("|" + x);
  } else {
    return x.toString();
  }
};

/** for quickly creating js Array */
export let js_array = (...xs: CalcitValue[]): CalcitValue[] => {
  return xs;
};

/** for..await alternative with function */
export let js_for_await = async (stream: AsyncIterableIterator<CalcitValue>, f: (v: CalcitValue) => Promise<CalcitValue>) => {
  if (typeof f !== "function") {
    throw new Error(`Expected function for: ${f}`);
  }
  let ret = null;
  for await (let item of stream) {
    ret = await f(item);
  }
  return ret;
};

export let _$n_js_object = (...xs: CalcitValue[]): Record<string, CalcitValue> => {
  if (xs.length % 2 !== 0) {
    throw new Error("&js-object expects even number of arguments");
  }
  var ret: Record<string, CalcitValue> = {}; // object
  let halfLength = xs.length >> 1;
  for (let idx = 0; idx < halfLength; idx++) {
    let k = xs[idx << 1];
    let v = xs[(idx << 1) + 1];
    if (typeof k === "string") {
      ret[k] = v;
    } else if (k instanceof CalcitTag) {
      ret[turn_string(k)] = v;
    } else {
      throw new Error("Invalid key for js Object");
    }
  }
  return ret;
};

export let _$o__$o_ = (tagName: CalcitValue, ...extra: CalcitValue[]): CalcitEnumValue => {
  return new CalcitEnumValue(tagName, extra, null);
};

export let _PCT__$o__$o_ = (enumPrototype: CalcitValue, tag: CalcitValue, ...extra: CalcitValue[]): CalcitEnumValue => {
  const proto = assert_enum_tag_args("%::", enumPrototype, tag as CalcitTag);
  const tagValue = tag as CalcitTag;

  const variantDefinition = proto.getOrNil(tagValue);
  if (variantDefinition === undefined) {
    throw new Error(`Enum ${proto.name.value} does not have variant ${tagValue.value}`);
  }

  if (variantDefinition instanceof CalcitSliceList) {
    const expectedArity = variantDefinition.len();
    const actualArity = extra.length;
    if (expectedArity !== actualArity) {
      throw new Error(`Variant ${tagValue.value} expects ${expectedArity} payload(s), but got ${actualArity}`);
    }
  } else {
    throw new Error(`Expected variant definition to be a list, got ${variantDefinition}`);
  }

  return new CalcitEnumValue(tag, extra, enumPrototype as CalcitEnumDef);
};

export let _PCT__PCT__$o__$o_ = (impl: CalcitValue, enumPrototype: CalcitValue, tag: CalcitValue, ...extra: CalcitValue[]): CalcitEnumValue => {
  // Runtime validation: check if tag exists in enum and arity matches
  const proto = assert_enum_tag_args("%%::", enumPrototype, tag as CalcitTag);
  const tagValue = tag as CalcitTag;

  const variantDefinition = proto.getOrNil(tagValue);
  if (variantDefinition === undefined) {
    throw new Error(`Enum ${proto.name.value} does not have variant ${tagValue.value}`);
  }

  if (variantDefinition instanceof CalcitSliceList) {
    const expectedArity = variantDefinition.len();
    const actualArity = extra.length;
    if (expectedArity !== actualArity) {
      throw new Error(`Variant ${tagValue.value} expects ${expectedArity} payload(s), but got ${actualArity}`);
    }
  } else {
    throw new Error(`Expected variant definition to be a list, got ${variantDefinition}`);
  }

  const implValue = coerce_impl(impl, "%:: with impl");
  return new CalcitEnumValue(tag, extra, (enumPrototype as CalcitEnumDef).withImpls(implValue));
};

// mutable place for core to register
type CalcitImplEntry = CalcitImpl | CalcitList | CalcitSliceList | null;

let calcit_builtin_impls = {
  number: null as CalcitImplEntry,
  string: null as CalcitImplEntry,
  set: null as CalcitImplEntry,
  list: null as CalcitImplEntry,
  map: null as CalcitImplEntry,
  fn: null as CalcitImplEntry,
  enum: null as CalcitImplEntry,
  struct: null as CalcitImplEntry,
  scalar: null as CalcitImplEntry,
};

// need to register code from outside
export let register_calcit_builtin_impls = (options: typeof calcit_builtin_impls) => {
  Object.assign(calcit_builtin_impls, options);
};

/** method used as closure */
export function invoke_method_closure(p: string) {
  const f = (obj: CalcitValue, ...args: CalcitValue[]) => {
    return invoke_method(p, obj, ...args);
  };
  (f as { __calcitMethodName?: string }).__calcitMethodName = p;
  return f;
}

function normalize_builtin_impls(entry: CalcitImplEntry): CalcitImpl[] | null {
  if (entry == null) return null;
  if (entry instanceof CalcitImpl) return [entry];
  if (entry instanceof CalcitList || entry instanceof CalcitSliceList) {
    return list_items(entry).map((item) => {
      if (item instanceof CalcitImpl) return item;
      throw new Error(`invoke-method expects impls in list, but received: ${toString(item, true)}`);
    }) as CalcitImpl[];
  }
  return null;
}

function lookup_impls(obj: CalcitValue): [CalcitImpl[], string] {
  let impls: CalcitImpl[];
  let tag: string;
  if (obj instanceof CalcitList || obj instanceof CalcitSliceList) {
    tag = "&core-list-methods";
    impls = normalize_builtin_impls(calcit_builtin_impls.list);
  } else if (obj instanceof CalcitMap || obj instanceof CalcitSliceMap) {
    tag = "&core-map-methods";
    impls = normalize_builtin_impls(calcit_builtin_impls.map);
  } else if (obj instanceof CalcitStructValue) {
    tag = obj.name.toString();
    let instanceImpls = obj.structRef.impls;
    let builtinRecordImpls = normalize_builtin_impls(calcit_builtin_impls.struct);
    if (builtinRecordImpls && instanceImpls && instanceImpls.length > 0) {
      impls = [...builtinRecordImpls, ...instanceImpls];
    } else if (builtinRecordImpls) {
      impls = builtinRecordImpls;
    } else {
      impls = instanceImpls;
    }
  } else if (obj instanceof CalcitEnumValue) {
    tag = obj.tag.toString();
    let instanceImpls = obj.impls;
    let builtinTupleImpls = normalize_builtin_impls(calcit_builtin_impls.enum);
    if (builtinTupleImpls && instanceImpls && instanceImpls.length > 0) {
      impls = [...builtinTupleImpls, ...instanceImpls];
    } else if (builtinTupleImpls) {
      impls = builtinTupleImpls;
    } else {
      impls = instanceImpls;
    }
  } else if (obj instanceof CalcitSet) {
    tag = "&core-set-methods";
    impls = normalize_builtin_impls(calcit_builtin_impls.set);
  } else if (obj instanceof CalcitStructDef) {
    // Bare type definitions (not yet instantiated) carry their own attached
    // impls, so introspection tools like `&methods-of` can answer "what
    // methods will instances of this type have" without a concrete instance.
    tag = obj.name.toString();
    const builtinRecordImpls = normalize_builtin_impls(calcit_builtin_impls.struct) ?? [];
    impls = [...builtinRecordImpls, ...(obj.impls ?? [])];
  } else if (obj instanceof CalcitEnumDef) {
    tag = obj.name();
    const builtinTupleImpls = normalize_builtin_impls(calcit_builtin_impls.enum) ?? [];
    impls = [...builtinTupleImpls, ...(obj.impls ?? [])];
  } else if (typeof obj === "number") {
    tag = "&core-number-methods";
    impls = normalize_builtin_impls(calcit_builtin_impls.number);
  } else if (typeof obj === "string") {
    tag = "&core-string-methods";
    impls = normalize_builtin_impls(calcit_builtin_impls.string);
  } else if (typeof obj === "function") {
    tag = "&core-fn-methods";
    impls = normalize_builtin_impls(calcit_builtin_impls.fn);
  } else if (
    obj == null ||
    typeof obj === "boolean" ||
    obj instanceof CalcitTag ||
    obj instanceof CalcitSymbol ||
    obj instanceof CalcitCirruQuote
  ) {
    tag = "&core-scalar-impls";
    impls = normalize_builtin_impls(calcit_builtin_impls.scalar);
  } else {
    return null;
  }
  if (impls == null) return null;
  return [impls, tag];
}

export function invoke_method(p: string, obj: CalcitValue, ...args: CalcitValue[]) {
  let pair = lookup_impls(obj);
  if (pair == null) {
    throw new Error(`No implementation for ${obj?.toString() || JSON.stringify(obj)} to lookup .${p}`);
  }
  let impls = pair[0];
  let tag = pair[1];
  // builtin impl lists are ordered by priority in calcit-core.
  // user-defined values use impl-traits append, so later impls override earlier ones.
  let reverse = obj instanceof CalcitStructValue || obj instanceof CalcitEnumValue || obj instanceof CalcitStructDef || obj instanceof CalcitEnumDef;
  let idx = reverse ? impls.length - 1 : 0;
  while (reverse ? idx >= 0 : idx < impls.length) {
    let klass = impls[idx];
    if (klass != null) {
      let method = klass.getOrNil(p);
      if (method != null) {
        if (typeof method !== "function") {
          throw new Error(`Method '.${p}' for '${tag}' is not a function: ${method}`);
        }
        return method(obj, ...args);
      }
    }
    idx += reverse ? -1 : 1;
  }
  throw new Error(`No method '.${p}' for '${tag}' object '${obj}'.`);
}

export function _$n_methods_of(obj: CalcitValue): CalcitSliceList {
  if (arguments.length !== 1) throw new Error("&methods-of expected 1 argument");
  // Traits declare methods directly rather than through attached impls.
  if (obj instanceof CalcitTrait) {
    return new CalcitSliceList(obj.methods.map((m) => invoke_method_closure(m.value)));
  }
  let pair = lookup_impls(obj);
  if (pair == null) {
    throw new Error(`&methods-of cannot resolve impls for: ${toString(obj, true)}`);
  }
  let impls = pair[0];
  let reverse = obj instanceof CalcitStructValue || obj instanceof CalcitEnumValue || obj instanceof CalcitStructDef || obj instanceof CalcitEnumDef;
  let seen = new Set<string>();
  let ys: CalcitValue[] = [];

  let idx = reverse ? impls.length - 1 : 0;
  while (reverse ? idx >= 0 : idx < impls.length) {
    let impl = impls[idx];
    if (impl != null) {
      for (let k = 0; k < impl.fields.length; k++) {
        let rawName = impl.fields[k].value;
        let name = "." + rawName;
        if (!seen.has(name)) {
          seen.add(name);
          ys.push(invoke_method_closure(rawName));
        }
      }
    }
    idx += reverse ? -1 : 1;
  }
  return new CalcitSliceList(ys);
}

export function _$n_inspect_methods(obj: CalcitValue, note: CalcitValue): CalcitValue {
  if (arguments.length !== 2) throw new Error("&inspect-methods expected 2 arguments");
  if (obj instanceof CalcitTrait) {
    console.log("\n&inspect-methods");
    console.log(`Note: ${toString(note, true)}`);
    console.log(`Value type: ${type_of(obj).toString()}`);
    console.log(`Value: ${toString(obj, true)}`);
    console.log("Method call syntax: `.method self p1 p2`");
    console.log("  - dot is part of the method name, first arg is the receiver\n");
    const names = obj.methods.map((m) => "." + m.value);
    console.log(`Trait methods declared directly (no impls): ${names.length}`);
    console.log(`\nAll methods (unique, high → low): ${names.length}`);
    console.log("  " + names.join(" "));
    console.log("\n");
    return obj;
  }
  let pair = lookup_impls(obj);
  if (pair == null) {
    throw new Error(`&inspect-methods cannot resolve impls for: ${toString(obj, true)}`);
  }
  let impls = pair[0];
  let reverse = obj instanceof CalcitStructValue || obj instanceof CalcitEnumValue || obj instanceof CalcitStructDef || obj instanceof CalcitEnumDef;

  console.log("\n&inspect-methods");
  console.log(`Note: ${toString(note, true)}`);
  console.log(`Value type: ${type_of(obj).toString()}`);
  console.log(`Value: ${toString(obj, true)}`);
  console.log("Method call syntax: `.method self p1 p2`");
  console.log("  - dot is part of the method name, first arg is the receiver\n");

  let implsInOrder: CalcitImpl[] = [];
  let idx = reverse ? impls.length - 1 : 0;
  while (reverse ? idx >= 0 : idx < impls.length) {
    let impl = impls[idx];
    if (impl != null) implsInOrder.push(impl);
    idx += reverse ? -1 : 1;
  }

  console.log(`Impls (high → low precedence): ${implsInOrder.length}`);
  for (let i = 0; i < implsInOrder.length; i++) {
    let impl = implsInOrder[i];
    let names: string[] = [];
    for (let k = 0; k < impl.fields.length; k++) {
      names.push("." + impl.fields[k].value);
    }
    const originName = impl.origin != null ? impl.origin.name.toString() : impl.name.toString();
    console.log(`  #${i}: ${originName}  (${names.join(" ")})`);
  }

  let ms = _$n_methods_of(obj);
  console.log(`\nAll methods (unique, high → low): ${ms.len()}`);
  console.log("  " + Array.from(ms.items()).map((m) => toString(m, false)).join(" "));
  console.log("\n");

  return obj;
}

export function _$n_trait_call(traitDef: CalcitValue, method: CalcitValue, obj: CalcitValue, ...args: CalcitValue[]) {
  if (arguments.length < 3) {
    throw new Error("&trait-call expected 3+ arguments (trait, method, receiver, & args)");
  }
  if (!(traitDef instanceof CalcitTrait)) {
    throw new Error(`&trait-call expected a trait definition as first argument, but received: ${toString(traitDef, true)}`);
  }
  const methodName = getStringName(method);
  const traitHasMethod = traitDef.methods.some((m) => m.value === methodName);
  if (!traitHasMethod) {
    const ms = traitDef.methods.map((m) => m.toString()).join(" ");
    throw new Error(`&trait-call: trait ${traitDef.name.toString()} does not define method :${methodName}. Available methods: ${ms}`);
  }
  const pair = lookup_impls(obj);
  if (pair == null) {
    throw new Error(`&trait-call cannot resolve impls for: ${toString(obj, true)}`);
  }
  const impls = pair[0];
  const reverse = obj instanceof CalcitStructValue || obj instanceof CalcitEnumValue || obj instanceof CalcitStructDef || obj instanceof CalcitEnumDef;
  let idx = reverse ? impls.length - 1 : 0;
  while (reverse ? idx >= 0 : idx < impls.length) {
    const impl = impls[idx];
    if (impl != null && impl.origin === traitDef) {
      const fn = impl.getOrNil(methodName);
      if (fn != null) {
        if (typeof fn !== "function") {
          throw new Error(`&trait-call: method :${methodName} for trait ${traitDef.name.toString()} is not a function: ${toString(fn, true)}`);
        }
        return fn(obj, ...args);
      }
    }
    idx += reverse ? -1 : 1;
  }
  throw new Error(
    `&trait-call: cannot find impl for trait ${traitDef.name.toString()} on ${toString(obj, true)}. Hint: use defimpl to create impls tagged by trait.`
  );
}

export let _$n_map_$o_to_list = (m: CalcitValue): CalcitSliceList => {
  if (m instanceof CalcitMap || m instanceof CalcitSliceMap) {
    let ys = [];
    let pairs = m.pairs();
    for (let idx = 0; idx < pairs.length; idx++) {
      let pair = pairs[idx];
      ys.push(new CalcitSliceList(pair));
    }
    return new CalcitSliceList(ys);
  } else {
    throw new Error("&map:to-list expected a Map");
  }
};

export let _$n_map_$o_diff_new = (a: CalcitValue, b: CalcitValue): CalcitMap => {
  if ((a instanceof CalcitMap || a instanceof CalcitSliceMap) && (b instanceof CalcitMap || b instanceof CalcitSliceMap)) {
    return a.diffNew(b);
  } else {
    throw new Error("expected 2 maps");
  }
};

export let _$n_map_$o_diff_keys = (a: CalcitValue, b: CalcitValue): CalcitSet => {
  if ((a instanceof CalcitMap || a instanceof CalcitSliceMap) && (b instanceof CalcitMap || b instanceof CalcitSliceMap)) {
    return a.diffKeys(b);
  } else {
    throw new Error("expected 2 maps");
  }
};

export let _$n_map_$o_common_keys = (a: CalcitValue, b: CalcitValue): CalcitSet => {
  if ((a instanceof CalcitMap || a instanceof CalcitSliceMap) && (b instanceof CalcitMap || b instanceof CalcitSliceMap)) {
    return a.commonKeys(b);
  } else {
    throw new Error("expected 2 maps");
  }
};

/** Single-pass diff: returns [drop-keys, new-diff, common-triples] in two traversals instead of 3+ */
export let _$n_map_$o_diff_triple = (a: CalcitValue, b: CalcitValue): CalcitSliceList => {
  if ((a instanceof CalcitMap || a instanceof CalcitSliceMap) && (b instanceof CalcitMap || b instanceof CalcitSliceMap)) {
    let dropKeys: CalcitValue[] = [];
    let commonTriples: CalcitValue[] = [];

    // One pass over a: split into drop-keys and common-triples
    let aKeys = a.keysArray();
    for (let i = 0; i < aKeys.length; i++) {
      let k = aKeys[i];
      if (b.contains(k)) {
        commonTriples.push(new CalcitSliceList([k, a.get(k), b.get(k)]));
      } else {
        dropKeys.push(k);
      }
    }

    // One pass over b: collect entries not in a
    let newDiffPairs: CalcitValue[] = [];
    let bKeys = b.keysArray();
    for (let i = 0; i < bKeys.length; i++) {
      let k = bKeys[i];
      if (!a.contains(k)) {
        newDiffPairs.push(k);
        newDiffPairs.push(b.get(k));
      }
    }

    return new CalcitSliceList([new CalcitSet(dropKeys), new CalcitSliceMap(newDiffPairs), new CalcitSliceList(commonTriples)]);
  } else {
    throw new Error("&map:diff-triple expected 2 maps");
  }
};

export let bit_shr = (base: number, step: number): number => {
  return base >> step;
};
export let bit_shl = (base: number, step: number): number => {
  return base << step;
};
export let bit_and = (a: number, b: number): number => {
  return a & b;
};
export let bit_or = (a: number, b: number): number => {
  return a | b;
};
export let bit_xor = (a: number, b: number): number => {
  return a ^ b;
};
export let bit_not = (a: number): number => {
  return ~a;
};

export let _$n_list_$o_to_set = (xs: CalcitList): CalcitSet => {
  var result: CalcitValue[] = [];
  let data = xs.toArray();
  for (let idx = 0; idx < data.length; idx++) {
    result.push(data[idx]);
  }
  return new CalcitSet(result);
};

export let _$n_list_$o_distinct = (xs: CalcitList): CalcitSliceList => {
  var result: CalcitValue[] = [];
  let data = xs.toArray();
  outer: for (let idx = 0; idx < data.length; idx++) {
    for (let j = 0; j < result.length; j++) {
      if (_$n__$e_(data[idx], result[j])) {
        continue outer;
      }
    }
    result.push(data[idx]);
  }
  return new CalcitSliceList(result);
};

export let _$n_str_$o_pad_left = (s: string, size: number, pattern: string): string => {
  return s.padStart(size, pattern);
};

export let _$n_str_$o_pad_right = (s: string, size: number, pattern: string): string => {
  return s.padEnd(size, pattern);
};

export let _$n_get_os = (): CalcitTag => {
  return newTag("js-engine");
};

export let _$n_buffer = (...xs: CalcitValue[]): Uint8Array => {
  let buf = new Uint8Array(xs.length);

  for (let idx = 0; idx < xs.length; idx++) {
    let x = xs[idx];
    if (typeof x === "number") {
      buf[idx] = x;
    } else if (typeof x === "string") {
      buf[idx] = parseInt(x, 16);
    } else {
      throw new Error("invalid value for buffer");
    }
  }

  return buf;
};

export let _$n_cirru_nth = (xs: CalcitCirruQuote, idx: number) => {
  if (xs instanceof CalcitCirruQuote) {
    return xs.nth(idx);
  } else {
    throw new Error("Expected a Cirru Quote");
  }
};

export let _$n_cirru_type = (xs: CalcitCirruQuote, idx: number) => {
  if (xs instanceof CalcitCirruQuote) {
    return Array.isArray(xs.value) ? newTag("list") : newTag("leaf");
  } else {
    throw new Error("Expected a Cirru Quote");
  }
};

export let _$n_hash = (x: CalcitValue): number => {
  return hashFunction(x);
};

export let _$n_cirru_quote_$o_to_list = (x: CalcitCirruQuote): CalcitValue => {
  return x.toList();
};

// special procs have to be defined manually
export let reduce = foldl;

let unavailableProc = (...xs: []) => {
  console.warn("NOT available for calcit-js");
};

// not available for calcit-js
export let _$n_reset_gensym_index_$x_ = unavailableProc;
export let gensym = unavailableProc;
export let macroexpand = unavailableProc;
export let macroexpand_all = unavailableProc;
export let _$n_get_calcit_running_mode = unavailableProc;
export let _$n_get_def_doc = unavailableProc;
export let _$n_get_def_schema = unavailableProc;
export let _$n_ffi_response_resolve = unavailableProc;
export let _$n_ffi_response_reject = unavailableProc;
export let _$n_ffi_task_cancel = unavailableProc;

// already handled in code emitter
export let raise = unavailableProc;
