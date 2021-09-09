// CALCIT VERSION
export const calcit_version = "0.4.30";

import { overwriteComparator, initTernaryTreeMap } from "@calcit/ternary-tree";
import { parse } from "@cirru/parser.ts";

import { CalcitValue } from "./js-primes";
import { CalcitSymbol, CalcitKeyword, CalcitRef, CalcitFn, CalcitRecur, kwd, refsRegistry, toString, getStringName, to_js_data, _$n__$e_ } from "./calcit-data";

import { fieldsEqual, CalcitRecord } from "./js-record";

export * from "./calcit-data";
export * from "./js-record";
export * from "./js-map";
export * from "./js-list";
export * from "./js-set";
export * from "./js-primes";
export * from "./js-tuple";
export * from "./custom-formatter";
export * from "./js-cirru";

import { CalcitList, foldl } from "./js-list";
import { CalcitMap } from "./js-map";
import { CalcitSet } from "./js-set";
import { CalcitTuple } from "./js-tuple";
import { to_calcit_data, extract_cirru_edn } from "./js-cirru";

let inNodeJs = typeof process !== "undefined" && process?.release?.name === "node";

export let type_of = (x: any): CalcitKeyword => {
  if (typeof x === "string") {
    return kwd("string");
  }
  if (typeof x === "number") {
    return kwd("number");
  }
  if (x instanceof CalcitKeyword) {
    return kwd("keyword");
  }
  if (x instanceof CalcitList) {
    return kwd("list");
  }
  if (x instanceof CalcitMap) {
    return kwd("map");
  }
  if (x == null) {
    return kwd("nil");
  }
  if (x instanceof CalcitRef) {
    return kwd("ref");
  }
  if (x instanceof CalcitTuple) {
    return kwd("tuple");
  }
  if (x instanceof CalcitSymbol) {
    return kwd("symbol");
  }
  if (x instanceof CalcitSet) {
    return kwd("set");
  }
  if (x instanceof CalcitRecord) {
    return kwd("record");
  }
  if (x === true || x === false) {
    return kwd("bool");
  }
  if (typeof x === "function") {
    if (x.isMacro) {
      // this is faked...
      return kwd("macro");
    }
    return kwd("fn");
  }
  if (typeof x === "object") {
    return kwd("js-object");
  }
  throw new Error(`Unknown data ${x}`);
};

export let print = (...xs: CalcitValue[]): void => {
  // TODO stringify each values
  console.log(xs.map((x) => toString(x, false)).join(" "));
};

export function _$n_list_$o_count(x: CalcitValue): number {
  if (x instanceof CalcitList) return x.len();

  throw new Error(`expected a list ${x}`);
}
export function _$n_str_$o_count(x: CalcitValue): number {
  if (typeof x === "string") return x.length;

  throw new Error(`expected a string ${x}`);
}
export function _$n_map_$o_count(x: CalcitValue): number {
  if (x instanceof CalcitMap) return x.len();

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

export let _$L_ = (...xs: CalcitValue[]): CalcitList => {
  return new CalcitList(xs);
};
// single quote as alias for list
export let _SQUO_ = (...xs: CalcitValue[]): CalcitList => {
  return new CalcitList(xs);
};

export let _$n__$M_ = (...xs: CalcitValue[]): CalcitMap => {
  if (xs.length % 2 !== 0) {
    throw new Error("&map expects even number of arguments");
  }
  return new CalcitMap(xs);
};

export let defatom = (path: string, x: CalcitValue): CalcitValue => {
  let v = new CalcitRef(x, path);
  refsRegistry.set(path, v);
  return v;
};

export let peekDefatom = (path: string): CalcitRef => {
  return refsRegistry.get(path);
};

export let deref = (x: CalcitRef): CalcitValue => {
  let a = refsRegistry.get(x.path);
  if (!(a instanceof CalcitRef)) {
    console.warn("Can not find ref:", x);
  }
  return a.value;
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
  if (xs instanceof CalcitList) {
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
  if (xs instanceof CalcitMap) return xs.contains(x);

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
  if (xs instanceof CalcitList) {
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
  if (xs instanceof CalcitMap) {
    for (let [k, v] of xs.pairs()) {
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

  if (xs instanceof CalcitList) return xs.get(k);

  throw new Error("Does not support `nth` on this type");
};

export let _$n_tuple_$o_nth = function (xs: CalcitValue, k: CalcitValue) {
  if (arguments.length !== 2) throw new Error("nth takes 2 arguments");
  if (typeof k !== "number") throw new Error("Expected number index for a list");

  if (xs instanceof CalcitTuple) return xs.get(k);

  throw new Error("Does not support `nth` on this type");
};

export let _$n_record_$o_nth = function (xs: CalcitValue, k: CalcitValue) {
  if (arguments.length !== 2) throw new Error("nth takes 2 arguments");
  if (typeof k !== "number") throw new Error("Expected number index for a list");

  if (xs instanceof CalcitRecord) {
    if (k < 0 || k >= xs.fields.length) {
      throw new Error("Out of bound");
    }
    return new CalcitList([kwd(xs.fields[k]), xs.values[k]]);
  }

  throw new Error("Does not support `nth` on this type");
};

export let _$n_record_$o_get = function (xs: CalcitValue, k: CalcitValue) {
  if (arguments.length !== 2) {
    throw new Error("record &get takes 2 arguments");
  }

  if (xs instanceof CalcitRecord) return xs.get(k);

  throw new Error("Does not support `&get` on this type");
};

export let _$n_list_$o_assoc = function (xs: CalcitValue, k: CalcitValue, v: CalcitValue) {
  if (arguments.length !== 3) throw new Error("assoc takes 3 arguments");

  if (xs instanceof CalcitList) {
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

  if (xs instanceof CalcitMap) return xs.assoc(...args);

  throw new Error("map `assoc` expected a map");
};
export let _$n_record_$o_assoc = function (xs: CalcitValue, k: CalcitValue, v: CalcitValue) {
  if (arguments.length !== 3) throw new Error("assoc takes 3 arguments");

  if (xs instanceof CalcitRecord) return xs.assoc(k, v);

  throw new Error("record `assoc` expected a record");
};

export let _$n_list_$o_assoc_before = function (xs: CalcitList, k: number, v: CalcitValue): CalcitList {
  if (arguments.length !== 3) {
    throw new Error("assoc takes 3 arguments");
  }
  if (xs instanceof CalcitList) {
    if (typeof k !== "number") {
      throw new Error("Expected number index for lists");
    }
    return xs.assocBefore(k, v);
  }

  throw new Error("Does not support `assoc-before` on this type");
};

export let _$n_list_$o_assoc_after = function (xs: CalcitList, k: number, v: CalcitValue): CalcitList {
  if (arguments.length !== 3) {
    throw new Error("assoc takes 3 arguments");
  }
  if (xs instanceof CalcitList) {
    if (typeof k !== "number") {
      throw new Error("Expected number index for lists");
    }
    return xs.assocAfter(k, v);
  }

  throw new Error("Does not support `assoc-after` on this type");
};

export let _$n_list_$o_dissoc = function (xs: CalcitValue, k: CalcitValue) {
  if (arguments.length !== 2) throw new Error("dissoc takes 2 arguments");

  if (xs instanceof CalcitList) {
    if (typeof k !== "number") throw new Error("Expected number index for lists");

    return xs.dissoc(k);
  }

  throw new Error("`dissoc` expected a list");
};
export let _$n_map_$o_dissoc = function (xs: CalcitValue, ...args: CalcitValue[]) {
  if (args.length < 1) throw new Error("dissoc takes at least 2 arguments");

  if (xs instanceof CalcitMap) {
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

export let add_watch = (a: CalcitRef, k: CalcitKeyword, f: CalcitFn): null => {
  if (!(a instanceof CalcitRef)) {
    throw new Error("Expected ref for add-watch!");
  }
  if (!(k instanceof CalcitKeyword)) {
    throw new Error("Expected watcher key in keyword");
  }
  if (!(typeof f === "function")) {
    throw new Error("Expected watcher function");
  }
  a.listeners.set(k, f);
  return null;
};

export let remove_watch = (a: CalcitRef, k: CalcitKeyword): null => {
  a.listeners.delete(k);
  return null;
};

export let range = (n: number, m: number, m2: number): CalcitList => {
  var result = new CalcitList([]);
  if (m2 != null) {
    console.warn("TODO range with 3 arguments"); // TODO
  }
  if (m != null) {
    var idx = n;
    while (idx < m) {
      result = result.append(idx);
      idx = idx + 1;
    }
  } else {
    var idx = 0;
    while (idx < n) {
      result = result.append(idx);
      idx = idx + 1;
    }
  }
  return result;
};

export function _$n_list_$o_empty_$q_(xs: CalcitValue): boolean {
  if (xs instanceof CalcitList) return xs.isEmpty();
  throw new Error(`expected a list ${xs}`);
}
export function _$n_str_$o_empty_$q_(xs: CalcitValue): boolean {
  if (typeof xs == "string") return xs.length == 0;
  throw new Error(`expected a string ${xs}`);
}
export function _$n_map_$o_empty_$q_(xs: CalcitValue): boolean {
  if (xs instanceof CalcitMap) return xs.isEmpty();

  throw new Error(`expected a list ${xs}`);
}
export function _$n_set_$o_empty_$q_(xs: CalcitValue): boolean {
  if (xs instanceof CalcitSet) return xs.len() === 0;
  throw new Error(`expected a list ${xs}`);
}

export let _$n_list_$o_first = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitList) {
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
export let _$n_map_$o_first = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitMap) {
    // TODO order may not be stable enough
    let ys = xs.pairs();
    if (ys.length > 0) {
      return new CalcitList(ys[0]);
    } else {
      return null;
    }
  }
  console.error(xs);
  throw new Error("Expected a map");
};
export let _$n_set_$o_first = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitSet) {
    return xs.first();
  }

  console.error(xs);
  throw new Error("Expected a set");
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
  if (xs instanceof CalcitList) {
    if (xs.len() === 0) {
      return null;
    }
    return xs.rest();
  }
  console.error(xs);
  throw new Error("Expected a list");
};

export let _$n_str_$o_rest = (xs: CalcitValue): CalcitValue => {
  if (typeof xs === "string") return xs.substr(1);

  console.error(xs);
  throw new Error("Expects a string");
};
export let _$n_set_$o_rest = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitSet) return xs.rest();

  console.error(xs);
  throw new Error("Expect a set");
};
export let _$n_map_$o_rest = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitMap) {
    if (xs.len() > 0) {
      let k0 = xs.pairs()[0][0];
      return xs.dissoc(k0);
    } else {
      return new CalcitMap(initTernaryTreeMap<CalcitValue, CalcitValue>([]));
    }
  }
  console.error(xs);
  throw new Error("Expected map");
};

export let recur = (...xs: CalcitValue[]): CalcitRecur => {
  return new CalcitRecur(xs);
};

export let _$n_get_calcit_backend = () => {
  return kwd("js");
};

export let not = (x: boolean): boolean => {
  return !x;
};

export let prepend = (xs: CalcitValue, v: CalcitValue): CalcitList => {
  if (!(xs instanceof CalcitList)) {
    throw new Error("Expected array");
  }
  return xs.prepend(v);
};

export let append = (xs: CalcitValue, v: CalcitValue): CalcitList => {
  if (!(xs instanceof CalcitList)) {
    throw new Error("Expected array");
  }
  return xs.append(v);
};

export let last = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitList) {
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
  if (xs instanceof CalcitList) {
    if (xs.len() === 0) {
      return null;
    }
    return xs.slice(0, xs.len() - 1);
  }
  if (typeof xs === "string") {
    return xs.substr(0, xs.length - 1);
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
  for (let idx in xs) {
    result.push(xs[idx]);
  }
  return new CalcitSet(result);
};

let idCounter = 0;

export let generate_id_$x_ = (): string => {
  // TODO use nanoid.. this code is wrong
  idCounter = idCounter + 1;
  return `gen_id_${idCounter}`;
};

export let _$n_display_stack = (): null => {
  console.trace();
  return null;
};

export let _$n_list_$o_slice = (xs: CalcitList, from: number, to: number): CalcitList => {
  if (xs == null) {
    return null;
  }
  let size = xs.len();
  if (to == null) {
    to = size;
  } else if (to <= from) {
    return new CalcitList([]);
  } else if (to > size) {
    to = size;
  }
  return xs.slice(from, to);
};

export let _$n_list_$o_concat = (...lists: CalcitList[]): CalcitList => {
  let result: CalcitList = new CalcitList([]);
  for (let item of lists) {
    if (item == null) {
      continue;
    }
    if (item instanceof CalcitList) {
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
  return a == Math.round(a);
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
export let sort = (xs: CalcitList, f: CalcitFn): CalcitList => {
  if (xs == null) {
    return null;
  }
  if (xs instanceof CalcitList) {
    let ys = xs.toArray();
    return new CalcitList(ys.sort(f as any));
  }
  throw new Error("Expected list");
};

export let rand = (n: number, m: number): number => {
  if (m != null) {
    return n + (m - n) * Math.random();
  }
  if (n != null) {
    return Math.random() * n;
  }
  return Math.random() * 100;
};

export let rand_int = (n: number, m: number): number => {
  if (m != null) {
    return Math.floor(n + Math.random() * (m - n));
  }
  if (n != null) {
    return Math.floor(Math.random() * n);
  }
  return Math.floor(Math.random() * 100);
};

export let floor = (n: number): number => {
  return Math.floor(n);
};

export let _$n_merge = (a: CalcitValue, b: CalcitMap): CalcitValue => {
  if (a == null) {
    return b;
  }
  if (b == null) {
    return a;
  }
  if (a instanceof CalcitMap) {
    if (b instanceof CalcitMap) {
      return a.merge(b);
    } else {
      throw new Error("Expected an argument of map");
    }
  }
  if (a instanceof CalcitRecord) {
    if (b instanceof CalcitMap) {
      let values = [];
      for (let item of a.values) {
        values.push(item);
      }
      for (let [k, v] of b.pairs()) {
        let field = getStringName(k);
        let idx = a.fields.indexOf(field);
        if (idx >= 0) {
          values[idx] = v;
        } else {
          throw new Error(`Cannot find field ${field} among (${a.fields.join(", ")})`);
        }
      }
      return new CalcitRecord(a.name, a.fields, values);
    }
  }
  throw new Error("Expected map or record");
};

export let _$n_merge_non_nil = (a: CalcitMap, b: CalcitMap): CalcitMap => {
  if (a == null) {
    return b;
  }
  if (b == null) {
    return a;
  }
  if (!(a instanceof CalcitMap)) {
    throw new Error("Expected map");
  }
  if (!(b instanceof CalcitMap)) {
    throw new Error("Expected map");
  }

  return a.mergeSkip(b, null);
};

export let to_pairs = (xs: CalcitValue): CalcitValue => {
  if (xs instanceof CalcitMap) {
    let result: Array<CalcitList> = [];
    for (let [k, v] of xs.pairs()) {
      result.push(new CalcitList([k, v]));
    }
    return new CalcitSet(result);
  } else if (xs instanceof CalcitRecord) {
    let arr_result: Array<CalcitList> = [];
    for (let idx in xs.fields) {
      arr_result.push(new CalcitList([kwd(xs.fields[idx]), xs.values[idx]]));
    }
    return new CalcitList(arr_result);
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

export let split = (xs: string, x: string): CalcitList => {
  return new CalcitList(xs.split(x));
};
export let split_lines = (xs: string): CalcitList => {
  return new CalcitList(xs.split("\n"));
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
    while (idx < size && buffer[idx] == c) {
      idx = idx + 1;
    }
    buffer = buffer.substring(idx);
    var size = buffer.length;
    var idx = size;
    while (idx > 1 && buffer[idx - 1] == c) {
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

export let get_char_code = (c: string): number => {
  if (typeof c !== "string" || c.length !== 1) {
    throw new Error("Expected a character");
  }
  return c.charCodeAt(0);
};

export let char_from_code = (n: number): string => {
  if (typeof n !== "number") throw new Error("Expected na integer");
  return String.fromCharCode(n);
};

export let re_matches = (content: string, re: string): boolean => {
  return new RegExp(re).test(content);
};

export let re_find_index = (content: string, re: string): number => {
  return content.search(new RegExp(re));
};

export let re_find_all = (content: string, re: string): CalcitList => {
  let ys = content.match(new RegExp(re, "g"));
  if (ys == null) {
    return new CalcitList([]);
  } else {
    return new CalcitList(ys);
  }
};

export let parse_json = (x: string): CalcitValue => {
  return to_calcit_data(JSON.parse(x), false);
};

export let stringify_json = (x: CalcitValue, addColon: boolean = false): string => {
  return JSON.stringify(to_js_data(x, addColon));
};

export let _$n_set_$o_to_list = (x: CalcitSet): CalcitList => {
  return new CalcitList(x.values());
};

export let aget = (x: any, name: string): any => {
  return x[name];
};
export let aset = (x: any, name: string, v: any): any => {
  return (x[name] = v);
};

export let get_env = (name: string): string => {
  let v = undefined;
  if (inNodeJs) {
    // only available for Node.js
    v = process.env[name];
  } else if (typeof URLSearchParams != null && typeof location != null) {
    v = new URLSearchParams(location.search).get(name);
  }
  if (v == null) {
    console.warn(`(get-env "${name}"): ${v}`);
  }
  return v;
};

export let turn_keyword = (x: CalcitValue): CalcitKeyword => {
  if (typeof x === "string") {
    return kwd(x);
  }
  if (x instanceof CalcitKeyword) {
    return x;
  }
  if (x instanceof CalcitSymbol) {
    return kwd(x.value);
  }
  console.error(x);
  throw new Error("Unexpected data for keyword");
};

export let turn_symbol = (x: CalcitValue): CalcitKeyword => {
  if (typeof x === "string") {
    return new CalcitSymbol(x);
  }
  if (x instanceof CalcitSymbol) {
    return x;
  }
  if (x instanceof CalcitKeyword) {
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
  if (x instanceof CalcitKeyword) {
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

export let starts_with_$q_ = (xs: string, y: string): boolean => {
  return xs.startsWith(y);
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

export let arrayToList = (xs: Array<CalcitValue>): CalcitList => {
  return new CalcitList(xs ?? []);
};

export let listToArray = (xs: CalcitList): Array<CalcitValue> => {
  if (xs == null) {
    return null;
  }
  if (xs instanceof CalcitList) {
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
export let keyword_$q_ = (x: CalcitValue): boolean => {
  return x instanceof CalcitKeyword;
};
export let map_$q_ = (x: CalcitValue): boolean => {
  return x instanceof CalcitMap;
};
export let list_$q_ = (x: CalcitValue): boolean => {
  return x instanceof CalcitList;
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

export let parse_cirru = (code: string): CalcitList => {
  return to_calcit_data(parse(code), true) as CalcitList;
};

export let parse_cirru_edn = (code: string) => {
  return extract_cirru_edn(parse(code)[0]);
};

/** return in seconds, like from Nim */
export let get_time_$x_ = () => {
  return Date.now() / 1000;
};

/** return in seconds, like from Nim,
 * notice Nim version is slightly different
 */
export let parse_time = (text: string) => {
  return new Date(text).valueOf() / 1000;
};

export let format_to_lisp = (x: CalcitValue): string => {
  if (x == null) {
    return "nil";
  } else if (x instanceof CalcitSymbol) {
    return x.value;
  } else if (x instanceof CalcitList) {
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
    } else if (k instanceof CalcitKeyword) {
      ret[turn_string(k)] = v;
    } else {
      throw new Error("Invalid key for js Object");
    }
  }
  return ret;
};

/** notice, Nim version of format-time takes format */
export let format_time = (timeSecNumber: number, format?: string): string => {
  if (format != null) {
    console.error("format of calcit-js not implemented");
  }
  return new Date(timeSecNumber * 1000).toISOString();
};

export let _$o__$o_ = (a: CalcitValue, b: CalcitValue): CalcitTuple => {
  return new CalcitTuple(a, b);
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

export function invoke_method(p: string) {
  return (obj: CalcitValue, ...args: CalcitValue[]) => {
    let klass: CalcitRecord;
    let value = obj;
    if (obj == null) {
      klass = calcit_builtin_classes.nil;
    } else if (obj instanceof CalcitTuple) {
      if (obj.fst instanceof CalcitRecord) {
        klass = obj.fst;
      } else {
        throw new Error("Method invoking expected a record as class");
      }
    } else if (typeof obj === "number") {
      klass = calcit_builtin_classes.number;
    } else if (typeof obj === "string") {
      klass = calcit_builtin_classes.string;
    } else if (typeof obj === "function") {
      klass = calcit_builtin_classes.fn;
    } else if (obj instanceof CalcitSet) {
      klass = calcit_builtin_classes.set;
    } else if (obj instanceof CalcitList) {
      klass = calcit_builtin_classes.list;
    } else if (obj instanceof CalcitRecord) {
      klass = calcit_builtin_classes.record;
    } else if (obj instanceof CalcitMap) {
      klass = calcit_builtin_classes.map;
    } else {
      if ((obj as any)[p] == null) {
        throw new Error(`Missing method \`${p}\` on object`);
      }
      return (obj as any)[p](...args); // trying to call native JavaScript method
    }
    if (klass == null) throw new Error("Cannot find class for this object for invoking");

    if (!klass.contains(p)) throw new Error(`Missing method '${p}' for object: ${obj}`);

    let method = klass.get(p);
    if (typeof method === "function") {
      return method(value, ...args);
    } else {
      throw new Error("Method for invoking is not a function");
    }
  };
}

export let _$n_map_$o_to_list = (m: CalcitValue): CalcitList => {
  if (m instanceof CalcitMap) {
    let ys = [];
    for (let pair of m.pairs()) {
      ys.push(new CalcitList(pair));
    }
    return new CalcitList(ys);
  } else {
    throw new Error("&map:to-list expected a Map");
  }
};

enum PseudoTypeIndex {
  nil,
  bool,
  number,
  symbol,
  keyword,
  string,
  ref,
  tuple,
  recur,
  list,
  set,
  map,
  record,
  fn,
}

let typeAsInt = (x: CalcitValue): number => {
  // based on order used in Ord traint
  if (x == null) return PseudoTypeIndex.nil;
  let t = typeof x;
  if (t === "boolean") return PseudoTypeIndex.bool;
  if (t === "number") return PseudoTypeIndex.number;
  if (x instanceof CalcitSymbol) return PseudoTypeIndex.symbol;
  if (x instanceof CalcitKeyword) return PseudoTypeIndex.keyword;
  if (t === "string") return PseudoTypeIndex.string;
  if (x instanceof CalcitRef) return PseudoTypeIndex.ref;
  if (x instanceof CalcitTuple) return PseudoTypeIndex.tuple;
  if (x instanceof CalcitRecur) return PseudoTypeIndex.recur;
  if (x instanceof CalcitList) return PseudoTypeIndex.list;
  if (x instanceof CalcitSet) return PseudoTypeIndex.set;
  if (x instanceof CalcitMap) return PseudoTypeIndex.map;
  if (x instanceof CalcitRecord) return PseudoTypeIndex.record;
  // proc, fn, macro, syntax, not distinguished
  if (t === "function") return PseudoTypeIndex.fn;
  throw new Error("unknown type to compare");
};

let rawCompare = (x: any, y: any): number => {
  if (x < y) {
    return -1;
  } else if (x > y) {
    return 1;
  } else {
    return 0;
  }
};

export let _$n_compare = (a: CalcitValue, b: CalcitValue): number => {
  let ta = typeAsInt(a);
  let tb = typeAsInt(b);
  if (ta === tb) {
    switch (ta) {
      case PseudoTypeIndex.nil:
        return 0;
      case PseudoTypeIndex.bool:
        return rawCompare(a, b);
      case PseudoTypeIndex.number:
        return rawCompare(a, b);
      case PseudoTypeIndex.symbol:
        return rawCompare(a, b);
      case PseudoTypeIndex.keyword:
        return rawCompare(a, b);
      case PseudoTypeIndex.string:
        return rawCompare(a, b);
      case PseudoTypeIndex.ref:
        return rawCompare((a as CalcitRef).path, (b as CalcitRef).path);
      default:
        // TODO, need more accurate solution
        if (a < b) {
          return -1;
        } else if (a > b) {
          return 1;
        } else {
          return 0;
        }
    }
  } else {
    return rawCompare(ta, tb);
  }
};

export let _$n_map_$o_diff_new = (a: CalcitValue, b: CalcitValue): CalcitMap => {
  if (a instanceof CalcitMap && b instanceof CalcitMap) {
    return a.diffNew(b);
  } else {
    throw new Error("expected 2 maps");
  }
};

export let _$n_map_$o_diff_keys = (a: CalcitValue, b: CalcitValue): CalcitSet => {
  if (a instanceof CalcitMap && b instanceof CalcitMap) {
    return a.diffKeys(b);
  } else {
    throw new Error("expected 2 maps");
  }
};

export let _$n_map_$o_common_keys = (a: CalcitValue, b: CalcitValue): CalcitSet => {
  if (a instanceof CalcitMap && b instanceof CalcitMap) {
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

export let _$n_list_$o_to_set = (xs: CalcitList): CalcitSet => {
  var result: CalcitValue[] = [];
  let data = xs.toArray();
  for (let idx = 0; idx < data.length; idx++) {
    result.push(data[idx]);
  }
  return new CalcitSet(result);
};

export let _$n_list_$o_distinct = (xs: CalcitList): CalcitList => {
  var result: CalcitValue[] = [];
  let data = xs.toArray();
  outer: for (let idx in data) {
    for (let j = 0; j < result.length; j++) {
      if (_$n__$e_(data[idx], result[j])) {
        continue outer;
      }
    }
    result.push(data[idx]);
  }
  return new CalcitList(result);
};

export let _$n_get_os = (): CalcitKeyword => {
  return kwd("js-engine");
};

// special procs have to be defined manually
export let reduce = foldl;

let unavailableProc = (...xs: []) => {
  console.warn("NOT available for calcit-js");
};

// not available for calcit-js
export let _$n_reset_gensym_index_$x_ = unavailableProc;
export let dbt__GT_point = unavailableProc;
export let dbt_digits = unavailableProc;
export let dual_balanced_ternary = unavailableProc;
export let gensym = unavailableProc;
export let macroexpand = unavailableProc;
export let macroexpand_all = unavailableProc;
export let _$n_get_calcit_running_mode = unavailableProc;

// already handled in code emitter
export let raise = unavailableProc;
