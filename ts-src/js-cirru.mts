import { overwriteComparator, initTernaryTreeMap } from "@calcit/ternary-tree";
import { CirruWriterNode, writeCirruCode } from "@cirru/writer.ts";

import { CalcitValue } from "./js-primes.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";
import { CalcitRecord } from "./js-record.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitSet } from "./js-set.mjs";
import { CalcitKeyword, CalcitSymbol, CalcitRecur, CalcitRef, kwd } from "./calcit-data.mjs";
import { CalcitTuple } from "./js-tuple.mjs";

type CirruEdnFormat = string | CirruEdnFormat[];

export let format_cirru = (data: CalcitList, useInline: boolean): string => {
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
  if (x instanceof CalcitList || x instanceof CalcitSliceList) {
    // TODO can be faster
    return (["[]"] as CirruEdnFormat[]).concat(x.toArray().map(to_cirru_edn));
  }
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) {
    let buffer: CirruEdnFormat = ["{}"];
    for (let [k, v] of x.pairs()) {
      buffer.push([to_cirru_edn(k), to_cirru_edn(v)]);
    }
    return buffer;
  }
  if (x instanceof CalcitRecord) {
    let buffer: CirruEdnFormat = ["%{}", x.name.toString()];
    for (let idx = 0; idx < x.fields.length; idx++) {
      buffer.push([x.fields[idx].toString(), to_cirru_edn(x.values[idx])]);
    }
    return buffer;
  }
  if (x instanceof CalcitSet) {
    let buffer: CirruEdnFormat = ["#{}"];
    let values = x.values();
    for (let idx = 0; idx < values.length; idx++) {
      let y = values[idx];
      buffer.push(to_cirru_edn(y));
    }
    return buffer;
  }
  if (x instanceof CalcitTuple) {
    if (x.fst instanceof CalcitSymbol && x.fst.value === "quote") {
      // turn `x.snd` with CalcitList into raw Cirru nodes, which is in plain Array
      return ["quote", toWriterNode(x.snd as any)] as CirruEdnFormat;
    } else if (x.fst instanceof CalcitRecord) {
      return ["::", x.fst.name.toString(), to_cirru_edn(x.snd)];
    } else {
      throw new Error(`Unsupported tag for EDN: ${x.fst}`);
    }
  }
  console.error(x);
  throw new Error("Unexpected data to to-cirru-edn");
};

/** makes sure we got string */
let extractFieldKwd = (x: string) => {
  if (x[0] === ":") {
    return kwd(x.slice(1));
  } else {
    return kwd(x);
  }
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
    if (x === "") {
      throw new Error("cannot be empty");
    }
    if (x[0] === "|" || x[0] === '"') {
      return x.slice(1);
    }
    if (x[0] === ":") {
      return kwd(x.slice(1));
    }
    if (x[0] === "'") {
      return new CalcitSymbol(x.slice(1));
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
      let result: Array<CalcitValue> = [];
      x.forEach((pair, idx) => {
        if (idx === 0) {
          return; // skip first `{}` symbol
        }
        if (pair instanceof Array && pair.length === 2) {
          result.push(extract_cirru_edn(pair[0]), extract_cirru_edn(pair[1]));
        } else {
          throw new Error("Expected pairs for map");
        }
      });
      return new CalcitSliceMap(result);
    }
    if (x[0] === "%{}") {
      let name = x[1];
      if (typeof name != "string") {
        throw new Error("Expected string for record name");
      }
      // put to entries first, sort and then...
      let entries: Array<[CalcitKeyword, CalcitValue]> = [];
      x.forEach((pair, idx) => {
        if (idx <= 1) {
          return; // skip %{} name
        }

        if (pair instanceof Array && pair.length === 2) {
          if (typeof pair[0] === "string") {
            entries.push([extractFieldKwd(pair[0]), extract_cirru_edn(pair[1])]);
          } else {
            throw new Error("Expected string as field");
          }
        } else {
          throw new Error("Expected pairs for map");
        }
      });
      entries.sort((a, b) => {
        return a[0].cmp(b[0]);
      });
      let fields: Array<CalcitKeyword> = [];
      let values: Array<CalcitValue> = [];

      for (let idx = 0; idx < entries.length; idx++) {
        fields.push(entries[idx][0]);
        values.push(entries[idx][1]);
      }
      return new CalcitRecord(extractFieldKwd(name), fields, values);
    }
    if (x[0] === "[]") {
      return new CalcitSliceList(x.slice(1).map(extract_cirru_edn));
    }
    if (x[0] === "#{}") {
      return new CalcitSet(x.slice(1).map(extract_cirru_edn));
    }
    if (x[0] === "do" && x.length === 2) {
      return extract_cirru_edn(x[1]);
    }
    if (x[0] === "quote") {
      if (x.length !== 2) {
        throw new Error("quote expects 1 argument");
      }
      return new CalcitTuple(new CalcitSymbol("quote"), to_calcit_data(x[1], true));
    }
    if (x[0] === "::") {
      if (x.length !== 3) {
        throw new Error("tuple expects 2 values");
      }
      return new CalcitTuple(extract_cirru_edn(x[1]), extract_cirru_edn(x[2]));
    }
  }
  console.error(x);
  throw new Error("Unexpected data from cirru-edn");
};

export let format_cirru_edn = (data: CalcitValue, useInline: boolean = true): string => {
  if (data == null) {
    return "\ndo nil" + "\n";
  }
  if (typeof data === "string") {
    return "\ndo " + to_cirru_edn(data) + "\n";
  }
  if (typeof data === "boolean") {
    return "\ndo " + to_cirru_edn(data) + "\n";
  }
  if (typeof data === "string") {
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
  if (x == null) return null;

  if (typeof x === "number") return x;

  if (typeof x === "string") {
    if (!noKeyword && x[0] === ":" && x.slice(1).match(/^[\w\d_\?\!\-]+$/)) {
      return kwd(x.slice(1));
    }
    return x;
  }
  if (x === true || x === false) return x;

  if (typeof x === "function") return x;

  if (Array.isArray(x)) {
    var result: any[] = [];
    x.forEach((v) => {
      result.push(to_calcit_data(v, noKeyword));
    });
    return new CalcitSliceList(result);
  }
  if (x instanceof Set) {
    let result: Array<CalcitValue> = [];
    x.forEach((v) => {
      result.push(to_calcit_data(v, noKeyword));
    });
    return new CalcitSet(result);
  }

  if (x instanceof CalcitList || x instanceof CalcitSliceList) return x;
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) return x;
  if (x instanceof CalcitSet) return x;
  if (x instanceof CalcitRecord) return x;
  if (x instanceof CalcitRecur) return x;
  if (x instanceof CalcitRef) return x;
  if (x instanceof CalcitKeyword) return x;
  if (x instanceof CalcitSymbol) return x;
  if (x instanceof CalcitTuple) return x;

  // detects object
  if (x === Object(x)) {
    let result: Array<CalcitValue> = [];
    Object.keys(x).forEach((k) => {
      result.push(to_calcit_data(k, noKeyword), to_calcit_data(x[k], noKeyword));
    });
    return new CalcitSliceMap(result);
  }

  console.error(x);
  throw new Error("Unexpected data for converting");
};

let toWriterNode = (xs: CalcitList | CalcitSliceList): CirruWriterNode => {
  if (typeof xs === "string") {
    return xs;
  }
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    return xs.toArray().map(toWriterNode);
  } else {
    throw new Error("Unexpected type for CirruWriteNode");
  }
};
