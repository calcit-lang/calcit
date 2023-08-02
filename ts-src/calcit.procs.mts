// CALCIT VERSION
export const calcit_version = "0.7.11";

import { parse, ICirruNode } from "@cirru/parser.ts";
import { writeCirruCode } from "@cirru/writer.ts";

import { CalcitValue } from "./js-primes.mjs";
import {
  CalcitSymbol,
  CalcitTag,
  CalcitRef,
  CalcitFn,
  CalcitRecur,
  newTag,
  refsRegistry,
  toString,
  getStringName,
  to_js_data,
  _$n__$e_,
  hashFunction,
} from "./calcit-data.mjs";

import { fieldsEqual, CalcitRecord } from "./js-record.mjs";

export * from "./calcit-data.mjs";
export * from "./js-record.mjs";
export * from "./js-map.mjs";
export * from "./js-list.mjs";
export * from "./js-set.mjs";
export * from "./js-primes.mjs";
export * from "./js-tuple.mjs";
export * from "./custom-formatter.mjs";
export * from "./js-cirru.mjs";
export { _$n_compare } from "./js-primes.mjs";

import { CalcitList, CalcitSliceList, foldl } from "./js-list.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitSet } from "./js-set.mjs";
import { CalcitTuple } from "./js-tuple.mjs";
import { to_calcit_data, extract_cirru_edn, CalcitCirruQuote } from "./js-cirru.mjs";

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
  if (x == null) {
    return newTag("nil");
  }
  if (x instanceof CalcitRef) {
    return newTag("ref");
  }
  if (x instanceof CalcitTuple) {
    return newTag("tuple");
  }
  if (x instanceof CalcitSymbol) {
    return newTag("symbol");
  }
  if (x instanceof CalcitSet) {
    return newTag("set");
  }
  if (x instanceof CalcitRecord) {
    return newTag("record");
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

export let print = (...xs: CalcitValue[]): void => {
  // TODO stringify each values
  console.log(xs.map((x) => toString(x, false)).join(" "));
};

export function _$n_list_$o_count(x: CalcitValue): number {
  if (x instanceof CalcitList || x instanceof CalcitSliceList) return x.len();

  throw new Error(`expected a list ${x}`);
}
export function _$n_str_$o_count(x: CalcitValue): number {
  if (typeof x === "string") return x.length;

  throw new Error(`expected a string ${x}`);
}
export function _$n_map_$o_count(x: CalcitValue): number {
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) return x.len();

  throw new Error(`expected a map ${x}`);
}
export function _$n_record_$o_count(x: CalcitValue): number {
  if (x instanceof CalcitRecord) return x.fields.length;

  throw new Error(`expected a record ${x}`);
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

var atomCounter = 0;

export let atom = (x: CalcitValue): CalcitValue => {
  atomCounter = atomCounter + 1;
  let v = new CalcitRef(x, `atom-${atomCounter}`);
  return v;
};

export let peekDefatom = (path: string): CalcitRef => {
  return refsRegistry.get(path);
};

export let deref = (x: CalcitRef): CalcitValue => {
  return x.value;
};

export let _$n__ADD_ = (x: number, y: number): number => {
  return x + y;
};

export let _$n__$s_ = (x: number, y: number): number => {
  return x * y;
};

export let _$n_str = (x: CalcitValue): string => {
  return `${x}`;
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

export let _$n_record_$o_contains_$q_ = (xs: CalcitValue, x: CalcitValue): boolean => {
  if (xs instanceof CalcitRecord) return xs.contains(x);

  throw new Error("record `contains?` expected a record");
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

export let _$n_tuple_$o_nth = function (xs: CalcitValue, k: CalcitValue) {
  if (arguments.length !== 2) throw new Error("nth takes 2 arguments");
  if (typeof k !== "number") throw new Error("Expected number index for a list");

  if (xs instanceof CalcitTuple) return xs.get(k);

  throw new Error("Does not support `nth` on this type");
};
export let _$n_tuple_$o_count = function (xs: CalcitValue) {
  if (arguments.length !== 1) throw new Error("&tuple:count takes 1 arguments");

  if (xs instanceof CalcitTuple) return xs.count();

  throw new Error("Does not support `count` on this type");
};

export let _$n_tuple_$o_class = function (x: CalcitTuple) {
  if (arguments.length !== 1) throw new Error("&tuple:class takes 1 argument");
  return x.klass;
};

export let _$n_tuple_$o_params = function (x: CalcitTuple) {
  if (arguments.length !== 1) throw new Error("&tuple:params takes 1 argument");
  return new CalcitSliceList(x.extra);
};

export let _$n_tuple_$o_with_class = function (x: CalcitTuple, y: CalcitRecord) {
  if (arguments.length !== 2) throw new Error("&tuple:with-class takes 2 arguments");
  if (!(x instanceof CalcitTuple)) throw new Error("&tuple:with-class expects a tuple");
  if (!(y instanceof CalcitRecord)) throw new Error("&tuple:with-class expects second argument in record");
  return new CalcitTuple(x.tag, x.extra, y);
};

export let _$n_record_$o_get = function (xs: CalcitValue, k: CalcitTag) {
  if (arguments.length !== 2) {
    throw new Error("record &get takes 2 arguments");
  }

  if (xs instanceof CalcitRecord) return xs.get(k);

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
export let _$n_tuple_$o_assoc = function (xs: CalcitValue, k: CalcitValue, v: CalcitValue) {
  if (arguments.length !== 3) throw new Error("assoc takes 3 arguments");

  if (xs instanceof CalcitTuple) {
    if (typeof k !== "number") {
      throw new Error("Expected number index for lists");
    }
    return xs.assoc(k, v);
  }

  throw new Error("tuple `assoc` expected a tuple");
};
export let _$n_map_$o_assoc = function (xs: CalcitValue, ...args: CalcitValue[]) {
  if (arguments.length < 3) throw new Error("assoc takes at least 3 arguments");
  if (args.length % 2 !== 0) throw new Error("assoc expected odd arguments");

  if (xs instanceof CalcitMap || xs instanceof CalcitSliceMap) return xs.assoc(...args);

  throw new Error("map `assoc` expected a map");
};
export let _$n_record_$o_assoc = function (xs: CalcitValue, k: CalcitValue, v: CalcitValue) {
  if (arguments.length !== 3) throw new Error("assoc takes 3 arguments");

  if (xs instanceof CalcitRecord) return xs.assoc(k, v);

  throw new Error("record `assoc` expected a record");
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

export let reset_$x_ = (a: CalcitRef, v: CalcitValue): null => {
  if (!(a instanceof CalcitRef)) {
    throw new Error("Expected ref for reset!");
  }
  let prev = a.value;
  a.value = v;
  for (let [k, f] of a.listeners) {
    f(v, prev);
  }
  return null;
};

export let add_watch = (a: CalcitRef, k: CalcitTag, f: CalcitFn): null => {
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
  return null;
};

export let remove_watch = (a: CalcitRef, k: CalcitTag): null => {
  a.listeners.delete(k);
  return null;
};

export let range = (n: number, m: number, step: number = 1): CalcitSliceList | CalcitList => {
  var result: CalcitList | CalcitSliceList = new CalcitSliceList([]);
  if (m != null) {
    var idx = n;
    while (idx < m) {
      result = result.append(idx);
      idx = idx + step;
    }
  } else {
    var idx = 0;
    while (idx < n) {
      result = result.append(idx);
      idx = idx + step;
    }
  }
  return result;
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

export let timeout_call = (duration: number, f: CalcitFn): null => {
  if (typeof duration !== "number") {
    throw new Error("Expected duration in number");
  }
  if (typeof f !== "function") {
    throw new Error("Expected callback in fn");
  }
  setTimeout(f, duration);
  return null;
};

export let _$n_list_$o_rest = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    if (xs.len() === 0) {
      return null;
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

export let butlast = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    if (xs.len() === 0) {
      return null;
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

export let _$n_display_stack = (): null => {
  console.trace();
  return null;
};

export let _$n_list_$o_slice = (xs: CalcitList, from: number, to: number): CalcitSliceList | CalcitList => {
  if (xs == null) {
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
    if (item == null) {
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
  if (xs == null) {
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
  let buffer = "";
  if (a != null) {
    buffer += toString(a, false);
  }
  if (b != null) {
    buffer += toString(b, false);
  }
  return buffer;
};
export let sort = (xs: CalcitList | CalcitSliceList, f: CalcitFn): CalcitSliceList => {
  if (xs == null) {
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
  if (a == null) {
    return b;
  }
  if (b == null) {
    return a;
  }
  if (a instanceof CalcitMap || a instanceof CalcitSliceMap) {
    if (b instanceof CalcitMap || b instanceof CalcitSliceMap) {
      return a.merge(b);
    } else {
      throw new Error("Expected an argument of map");
    }
  }
  if (a instanceof CalcitRecord) {
    if (b instanceof CalcitMap || b instanceof CalcitSliceMap) {
      let values = [];
      for (let idx = 0; idx < a.values.length; idx++) {
        values.push(a.values[idx]);
      }
      let pairs = b.pairs();
      for (let idx = 0; idx < pairs.length; idx++) {
        let [k, v] = pairs[idx];
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
      return new CalcitRecord(a.name, a.fields, values);
    }
  }
  throw new Error("Expected map or record");
};

export let _$n_merge_non_nil = (a: CalcitMap | CalcitSliceMap, b: CalcitMap | CalcitSliceMap): CalcitMap | CalcitSliceMap => {
  if (a == null) {
    return b;
  }
  if (b == null) {
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
  } else if (xs instanceof CalcitRecord) {
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
  if (y == null) {
    return xs;
  }
  return xs.include(y);
};

export let _$n_exclude = (xs: CalcitSet, y: CalcitValue): CalcitSet => {
  if (!(xs instanceof CalcitSet)) {
    throw new Error("Expected a set");
  }
  if (y == null) {
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
  var result = x;
  while (result.indexOf(y) >= 0) {
    result = result.replace(y, z);
  }
  return result;
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

export let parse_float = (x: string): number => {
  return parseFloat(x);
};
export let trim = (x: string, c: string): string => {
  if (c != null) {
    if (c.length !== 1) {
      throw new Error("Expceted c of a character");
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

export let get_env = (name: string, v0: string): string => {
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
  return v ?? v0;
};

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

export let pr_str = (...args: CalcitValue[]): string => {
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
  if (x == null) {
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
  throw new Error("expected strings or tags");
};
export let ends_with_$q_ = (xs: string, y: string): boolean => {
  return xs.endsWith(y);
};

export let blank_$q_ = (x: string): boolean => {
  if (x == null) {
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
  if (xs == null) {
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
  return x == null;
};
export let tag_$q_ = (x: CalcitValue): boolean => {
  return x instanceof CalcitTag;
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
export let record_$q_ = (x: CalcitValue): boolean => {
  return x instanceof CalcitRecord;
};
export let tuple_$q_ = (x: CalcitValue): boolean => {
  return x instanceof CalcitTuple;
};
export let buffer_$q_ = (x: CalcitValue): boolean => {
  console.warn("TODO, detecting buffer");
  return false;
};

export let _$n_str_$o_escape = (x: string) => JSON.stringify(x);

export let read_file = (path: string): string => {
  if (inNodeJs) {
    // TODO
    (globalThis as any)["__calcit_injections__"].read_file(path);
  } else {
    // no actual File API in browser
    return localStorage.get(path) ?? "";
  }
};
export let write_file = (path: string, content: string): void => {
  if (inNodeJs) {
    // TODO
    (globalThis as any)["__calcit_injections__"].write_file(path, content);
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

export let parse_cirru_edn = (code: string) => {
  return extract_cirru_edn(parse(code)[0]);
};

export let format_to_lisp = (x: CalcitValue): string => {
  if (x == null) {
    return "nil";
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
  if (x == null) {
    return "nil";
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

export let _$o__$o_ = (tagName: CalcitValue, ...extra: CalcitValue[]): CalcitTuple => {
  let klass = new CalcitRecord(newTag("base-class"), [], []);
  return new CalcitTuple(tagName, extra, klass);
};

export let _PCT__$o__$o_ = (klass: CalcitRecord, tag: CalcitValue, ...extra: CalcitValue[]): CalcitTuple => {
  return new CalcitTuple(tag, extra, klass);
};

// mutable place for core to register
let calcit_builtin_classes = {
  number: null as CalcitRecord,
  string: null as CalcitRecord,
  set: null as CalcitRecord,
  list: null as CalcitRecord,
  map: null as CalcitRecord,
  record: null as CalcitRecord,
  nil: null as CalcitRecord,
  fn: null as CalcitRecord,
};

// need to register code from outside
export let register_calcit_builtin_classes = (options: typeof calcit_builtin_classes) => {
  Object.assign(calcit_builtin_classes, options);
};

/** method used as closure */
export function invoke_method_closure(p: string) {
  return (obj: CalcitValue, ...args: CalcitValue[]) => {
    return invoke_method(p, obj, ...args);
  };
}

export function invoke_method(p: string, obj: CalcitValue, ...args: CalcitValue[]) {
  let klass: CalcitRecord;
  let tag: string;
  let value = obj;
  if (obj == null) {
    tag = "&core-nil-class";
    klass = calcit_builtin_classes.nil;
  } else if (obj instanceof CalcitTuple) {
    if (obj.klass instanceof CalcitRecord) {
      tag = obj.tag.toString();
      klass = obj.klass;
    } else {
      throw new Error("Method invoking expected a record as class");
    }
  } else if (typeof obj === "number") {
    tag = "&core-number-class";
    klass = calcit_builtin_classes.number;
  } else if (typeof obj === "string") {
    tag = "&core-string-class";
    klass = calcit_builtin_classes.string;
  } else if (typeof obj === "function") {
    tag = "&core-fn-class";
    klass = calcit_builtin_classes.fn;
  } else if (obj instanceof CalcitSet) {
    tag = "&core-set-class";
    klass = calcit_builtin_classes.set;
  } else if (obj instanceof CalcitList || obj instanceof CalcitSliceList) {
    tag = "&core-list-class";
    klass = calcit_builtin_classes.list;
  } else if (obj instanceof CalcitRecord) {
    tag = "&core-record-class";
    klass = calcit_builtin_classes.record;
  } else if (obj instanceof CalcitMap || obj instanceof CalcitSliceMap) {
    tag = "&core-map-class";
    klass = calcit_builtin_classes.map;
  } else {
    if ((obj as any)[p] == null) {
      throw new Error(`Missing method \`${p}\` on object`);
    }
    return (obj as any)[p](...args); // trying to call native JavaScript method
  }
  if (klass == null) throw new Error("Cannot find class for this object for invoking");

  if (!klass.contains(p)) {
    throw new Error(`Missing method '.${p}' for '${tag}' object '${obj}'.\navailable fields are: ${klass.fields.map((fd: CalcitTag) => fd.value).join(" ")}`);
  }

  let method = klass.get(p);
  if (typeof method === "function") {
    return method(value, ...args);
  } else {
    throw new Error("Method for invoking is not a function");
  }
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

// already handled in code emitter
export let raise = unavailableProc;
