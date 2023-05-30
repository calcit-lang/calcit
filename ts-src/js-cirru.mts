import { overwriteComparator, initTernaryTreeMap } from "@calcit/ternary-tree";
import { CirruWriterNode, writeCirruCode } from "@cirru/writer.ts";

import { CalcitValue, is_literal, _$n_compare } from "./js-primes.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";
import { CalcitRecord } from "./js-record.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitSet } from "./js-set.mjs";
import { CalcitTag, CalcitSymbol, CalcitRecur, CalcitRef, newTag } from "./calcit-data.mjs";
import { CalcitTuple } from "./js-tuple.mjs";

type CirruEdnFormat = string | CirruEdnFormat[];

export class CalcitCirruQuote {
  value: CirruWriterNode;
  constructor(value: CirruWriterNode) {
    this.value = value;
  }
  toString(): string {
    return `(&cirru-quote ${JSON.stringify(this.value)})`;
  }
  toList(): CalcitValue {
    return to_calcit_data(this.value, true);
  }
  /** provide a simple text representation in Console or std out, with indentations */
  textForm(): string {
    return writeCirruCode(this.value);
  }
}

export let format_cirru = (data: CalcitCirruQuote | CalcitList, useInline: boolean): string => {
  if (data instanceof CalcitCirruQuote) {
    return writeCirruCode(data.value, { useInline });
  }
  let chunk = toWriterNode(data);
  if (!Array.isArray(chunk)) {
    throw new Error("Expected data of list");
  }
  for (let idx = 0; idx < chunk.length; idx++) {
    let item = chunk[idx];
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
  if (x instanceof CalcitTag) {
    return x.toString();
  }
  if (x instanceof CalcitSymbol) {
    return x.toString();
  }
  if (x instanceof CalcitList || x instanceof CalcitSliceList) {
    // TODO can be faster
    return (["[]"] as CirruEdnFormat[]).concat(x.toArray().map(to_cirru_edn));
  }
  if (x instanceof CalcitCirruQuote) {
    return ["quote", x.value];
  }
  if (x instanceof CalcitMap || x instanceof CalcitSliceMap) {
    let buffer: CirruEdnFormat = ["{}"];
    let pairs_buffer: [CalcitValue, CalcitValue][] = [];
    let pairs = x.pairs();
    for (let idx = 0; idx < pairs.length; idx++) {
      pairs_buffer.push(pairs[idx]);
    }
    pairs_buffer.sort((a, b) => {
      let a0_literal = is_literal(a[0]);
      let a1_literal = is_literal(a[1]);
      let b0_literal = is_literal(b[0]);
      let b1_literal = is_literal(b[1]);
      if (a0_literal && b0_literal) {
        if (a1_literal && !b1_literal) {
          return -1;
        } else if (!a1_literal && b1_literal) {
          return 1;
        } else {
          return _$n_compare(a[0], b[0]);
        }
      } else if (a0_literal && !b0_literal) {
        return -1;
      } else if (!a0_literal && b0_literal) {
        return 1;
      } else {
        return _$n_compare(a[0], b[0]);
      }
    });
    for (let idx = 0; idx < pairs_buffer.length; idx++) {
      let [k, v] = pairs_buffer[idx];
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
    values.sort((a, b) => {
      return _$n_compare(a, b);
    });
    for (let idx = 0; idx < values.length; idx++) {
      let y = values[idx];
      buffer.push(to_cirru_edn(y));
    }
    return buffer;
  }
  if (x instanceof CalcitTuple) {
    if (x.tag instanceof CalcitSymbol && x.tag.value === "quote") {
      // turn `x.snd` with CalcitList into raw Cirru nodes, which is in plain Array
      return ["quote", toWriterNode(x.get(1) as any)] as CirruEdnFormat;
    } else if (x.tag instanceof CalcitRecord) {
      return ["::", x.tag.name.toString(), to_cirru_edn(x.get(1))];
    } else {
      throw new Error(`Unsupported tag for EDN: ${x.tag}`);
    }
  }
  console.error(x);
  throw new Error("Unexpected data to to-cirru-edn");
};

/** makes sure we got string */
let extractFieldTag = (x: string) => {
  if (x[0] === ":") {
    return newTag(x.slice(1));
  } else {
    return newTag(x);
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
      return newTag(x.slice(1));
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
      let entries: Array<[CalcitTag, CalcitValue]> = [];
      x.forEach((pair, idx) => {
        if (idx <= 1) {
          return; // skip %{} name
        }

        if (pair instanceof Array && pair.length === 2) {
          if (typeof pair[0] === "string") {
            entries.push([extractFieldTag(pair[0]), extract_cirru_edn(pair[1])]);
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
      let fields: Array<CalcitTag> = [];
      let values: Array<CalcitValue> = [];

      for (let idx = 0; idx < entries.length; idx++) {
        fields.push(entries[idx][0]);
        values.push(entries[idx][1]);
      }
      return new CalcitRecord(extractFieldTag(name), fields, values);
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
      return new CalcitCirruQuote(x[1]);
    }
    if (x[0] === "::") {
      if (x.length < 3) {
        throw new Error("tuple expects at least 2 values");
      }
      let baseClass = new CalcitRecord(newTag("base-class"), [], []);
      return new CalcitTuple(extract_cirru_edn(x[1]), x.slice(2).map(extract_cirru_edn), baseClass);
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
  if (data instanceof CalcitTag) {
    return "\ndo " + to_cirru_edn(data) + "\n";
  }
  return writeCirruCode([to_cirru_edn(data)], { useInline: useInline });
};

export let to_calcit_data = (x: any, noKeyword: boolean = false): CalcitValue => {
  if (x == null) return null;

  if (typeof x === "number") return x;

  if (typeof x === "string") {
    if (!noKeyword && x[0] === ":" && x.slice(1).match(/^[\w\d_\?\!\-]+$/)) {
      return newTag(x.slice(1));
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
  if (x instanceof CalcitTag) return x;
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

  console.error("Unexpected data for converting", x);
  return null;
};

let toWriterNode = (xs: CalcitList | CalcitSliceList | Array<any> | String): CirruWriterNode => {
  if (typeof xs === "string") {
    return xs;
  } else if (Array.isArray(xs)) {
    return xs.map(toWriterNode);
  }
  if (xs instanceof CalcitList || xs instanceof CalcitSliceList) {
    return (xs.toArray() as Array<any>).map(toWriterNode);
  } else {
    throw new Error("Unexpected type for CirruWriteNode");
  }
};

/** deep compare cirru array */
export let cirru_deep_equal = (x: CirruWriterNode, y: CirruWriterNode): boolean => {
  if (x === y) {
    return true;
  } else if (Array.isArray(x) && Array.isArray(y)) {
    if (x.length !== y.length) {
      return false;
    }
    for (let idx = 0; idx < x.length; idx++) {
      if (!cirru_deep_equal(x[idx], y[idx])) {
        return false;
      }
    }
    return true;
  } else {
    return false;
  }
};
