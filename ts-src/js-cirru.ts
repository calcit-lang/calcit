import { overwriteComparator, initTernaryTreeMap } from "@calcit/ternary-tree";
import { CirruWriterNode, writeCirruCode } from "@cirru/writer.ts";

import { CalcitValue } from "./js-primes";
import { CalcitList } from "./js-list";
import { CalcitRecord } from "./js-record";
import { CalcitMap } from "./js-map";
import { CalcitSet } from "./js-set";
import { CalcitKeyword, CalcitSymbol, kwd } from "./calcit-data";

type CirruEdnFormat = string | CirruEdnFormat[];

export let write_cirru = (data: CalcitList, useInline: boolean): string => {
  let chunk = toWriterNode(data);
  if (!Array.isArray(chunk)) {
    throw new Error("Expected data of list");
  }
  for (let item of chunk) {
    if (!Array.isArray(item)) {
      throw new Error("Expected data in a list of lists");
    }
  }
  return writeCirruCode(chunk, { useInline });
};

/** better use string version of Cirru EDN in future */
export let to_cirru_edn = (x: CalcitValue): CirruEdnFormat => {
  if (x == null) {
    return "nil";
  }
  if (typeof x === "string") {
    return `|${x}`;
  }
  if (typeof x === "number") {
    return x.toString();
  }
  if (typeof x === "boolean") {
    return x.toString();
  }
  if (x instanceof CalcitKeyword) {
    return x.toString();
  }
  if (x instanceof CalcitSymbol) {
    return x.toString();
  }
  if (x instanceof CalcitList) {
    // TODO can be faster
    return (["[]"] as CirruEdnFormat[]).concat(x.toArray().map(to_cirru_edn));
  }
  if (x instanceof CalcitMap) {
    let buffer: CirruEdnFormat = ["{}"];
    for (let [k, v] of x.pairs()) {
      buffer.push([to_cirru_edn(k), to_cirru_edn(v)]);
    }
    return buffer;
  }
  if (x instanceof CalcitRecord) {
    let result: Record<string, CalcitValue> = {};
    let buffer: CirruEdnFormat = ["%{}", x.name];
    for (let idx in x.fields) {
      buffer.push([x.fields[idx], to_cirru_edn(x.values[idx])]);
    }
    return buffer;
  }
  if (x instanceof CalcitSet) {
    let buffer: CirruEdnFormat = ["#{}"];
    for (let y of x.value) {
      buffer.push(to_cirru_edn(y));
    }
    return buffer;
  }
  console.error(x);
  throw new Error("Unexpected data to to-cirru-edn");
};

export let extract_cirru_edn = (x: CirruEdnFormat): CalcitValue => {
  if (typeof x === "string") {
    if (x === "nil") {
      return null;
    }
    if (x === "true") {
      return true;
    }
    if (x === "false") {
      return false;
    }
    if (x == "") {
      throw new Error("cannot be empty");
    }
    if (x[0] === "|" || x[0] === '"') {
      return x.slice(1);
    }
    if (x[0] === ":") {
      return kwd(x.substr(1));
    }
    if (x[0] === "'") {
      return new CalcitSymbol(x.substr(1));
    }
    if (x.match(/^(-?)\d+(\.\d*$)?/)) {
      return parseFloat(x);
    }
    // allow things cannot be parsed accepted as raw strings
    // turned on since Cirru nodes passed from macros uses this
    return x;
  }
  if (x instanceof Array) {
    if (x.length === 0) {
      throw new Error("Cannot be empty");
    }
    if (x[0] === "{}") {
      let result: Array<[CalcitValue, CalcitValue]> = [];
      x.forEach((pair, idx) => {
        if (idx == 0) {
          return; // skip first `{}` symbol
        }
        if (pair instanceof Array && pair.length == 2) {
          result.push([extract_cirru_edn(pair[0]), extract_cirru_edn(pair[1])]);
        } else {
          throw new Error("Expected pairs for map");
        }
      });
      return new CalcitMap(initTernaryTreeMap(result));
    }
    if (x[0] === "%{}") {
      let name = x[1];
      if (typeof name != "string") {
        throw new Error("Expected string for record name");
      }
      let fields: Array<string> = [];
      let values: Array<CalcitValue> = [];
      x.forEach((pair, idx) => {
        if (idx <= 1) {
          return; // skip %{} name
        }

        if (pair instanceof Array && pair.length == 2) {
          if (typeof pair[0] === "string") {
            fields.push(pair[0]);
          } else {
            throw new Error("Expected string as field");
          }
          values.push(extract_cirru_edn(pair[1]));
        } else {
          throw new Error("Expected pairs for map");
        }
      });
      return new CalcitRecord(name, fields, values);
    }
    if (x[0] === "[]") {
      return new CalcitList(x.slice(1).map(extract_cirru_edn));
    }
    if (x[0] === "#{}") {
      return new CalcitSet(new Set(x.slice(1).map(extract_cirru_edn)));
    }
    if (x[0] === "do" && x.length === 2) {
      return extract_cirru_edn(x[1]);
    }
    if (x[0] === "quote") {
      if (x.length !== 2) {
        throw new Error("quote expects 1 argument");
      }
      return to_calcit_data(x[1], true);
    }
  }
  console.error(x);
  throw new Error("Unexpected data from cirru-edn");
};

export let write_cirru_edn = (data: CalcitValue, useInline: boolean = true): string => {
  if (data == null) {
    return "\ndo nil" + "\n";
  }
  if (typeof data === "string") {
    return "\ndo " + to_cirru_edn(data) + "\n";
  }
  if (typeof data == "boolean") {
    return "\ndo " + to_cirru_edn(data) + "\n";
  }
  if (typeof data == "string") {
    return "\ndo " + to_cirru_edn(data) + "\n";
  }
  if (data instanceof CalcitSymbol) {
    return "\ndo " + to_cirru_edn(data) + "\n";
  }
  if (data instanceof CalcitKeyword) {
    return "\ndo " + to_cirru_edn(data) + "\n";
  }
  return writeCirruCode([to_cirru_edn(data)], { useInline: useInline });
};

export let to_calcit_data = (x: any, noKeyword: boolean = false): CalcitValue => {
  if (x == null) {
    return null;
  }
  if (typeof x === "number") {
    return x;
  }
  if (typeof x === "string") {
    if (!noKeyword && x[0] === ":" && x.slice(1).match(/^[\w\d_\?\!\-]+$/)) {
      return kwd(x.slice(1));
    }
    return x;
  }
  if (x === true || x === false) {
    return x;
  }
  if (typeof x === "function") {
    return x;
  }
  if (Array.isArray(x)) {
    var result: any[] = [];
    x.forEach((v) => {
      result.push(to_calcit_data(v, noKeyword));
    });
    return new CalcitList(result);
  }
  if (x instanceof Set) {
    let result: Set<CalcitValue> = new Set();
    x.forEach((v) => {
      result.add(to_calcit_data(v, noKeyword));
    });
    return new CalcitSet(result);
  }
  // detects object
  if (x === Object(x)) {
    let result: Array<[CalcitValue, CalcitValue]> = [];
    Object.keys(x).forEach((k) => {
      result.push([to_calcit_data(k, noKeyword), to_calcit_data(x[k], noKeyword)]);
    });
    return new CalcitMap(initTernaryTreeMap(result));
  }

  console.error(x);
  throw new Error("Unexpected data for converting");
};

let toWriterNode = (xs: CalcitList): CirruWriterNode => {
  if (typeof xs === "string") {
    return xs;
  }
  if (xs instanceof CalcitList) {
    return xs.toArray().map(toWriterNode);
  } else {
    throw new Error("Unexpected type for CirruWriteNode");
  }
};
