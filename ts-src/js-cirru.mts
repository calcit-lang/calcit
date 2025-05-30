import { overwriteComparator, initTernaryTreeMap } from "@calcit/ternary-tree";
import { CirruWriterNode, writeCirruCode } from "@cirru/writer.ts";

import { CalcitValue, isLiteral, _$n_compare } from "./js-primes.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";
import { CalcitRecord } from "./js-record.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitSet } from "./js-set.mjs";
import { CalcitTag, CalcitSymbol, CalcitRecur, newTag } from "./calcit-data.mjs";
import { CalcitTuple } from "./js-tuple.mjs";
import { CalcitRef } from "./js-ref.mjs";
import { deepEqual } from "@calcit/ternary-tree/lib/utils.mjs";
import { atom } from "./js-ref.mjs";

type CirruEdnFormat = string | CirruEdnFormat[];

export class CalcitCirruQuote {
  value: CirruWriterNode;
  constructor(value: CirruWriterNode) {
    if (value == null) {
      throw new Error("cirru node cannot be null");
    }
    this.value = value;
  }
  toString(): string {
    return `(&cirru-quote ${JSON.stringify(this.value)})`;
  }
  toList(): CalcitValue {
    return to_calcit_data(this.value, true);
  }
  nth(idx: number): CalcitValue {
    if (Array.isArray(this.value)) {
      if (idx < this.value.length) {
        return new CalcitCirruQuote(this.value[idx]);
      } else {
        throw new Error(`nth out of range: ${idx}`);
      }
    } else {
      throw new Error(`&cirru-nth does not read into a string: ${this.value}`);
    }
  }
  /** provide a simple text representation in Console or std out, with indentations */
  textForm(): string {
    if (Array.isArray(this.value) && this.value.every((x) => Array.isArray(x))) {
      return writeCirruCode(this.value);
    } else {
      return this.toString();
    }
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
    return `${x}`;
  }
  if (typeof x === "boolean") {
    return `${x}`;
  }
  if (x instanceof CalcitTag) {
    return x.toString();
  }
  if (x instanceof CalcitSymbol) {
    return x.toString();
  }
  if (x instanceof CalcitList || x instanceof CalcitSliceList) {
    let ret: CirruEdnFormat[] = ["[]"];
    let arr = x.toArray();
    for (let idx = 0; idx < arr.length; idx++) {
      ret.push(to_cirru_edn(arr[idx]));
    }
    return ret;
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
      let a0_literal = isLiteral(a[0]);
      let a1_literal = isLiteral(a[1]);
      let b0_literal = isLiteral(b[0]);
      let b1_literal = isLiteral(b[1]);
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
      let k = pairs_buffer[idx][0];
      let v = pairs_buffer[idx][1];
      buffer.push([to_cirru_edn(k), to_cirru_edn(v)]);
    }
    return buffer;
  }
  if (x instanceof CalcitRecord) {
    let buffer: [string, CirruEdnFormat][] = [];
    for (let idx = 0; idx < x.fields.length; idx++) {
      buffer.push([x.fields[idx].toString(), to_cirru_edn(x.values[idx])]);
    }
    // placed literals first
    buffer.sort(recordFieldOrder);
    (buffer as any[]).unshift(x.name.toString());
    (buffer as any[]).unshift("%{}");
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
    } else if (x.tag instanceof CalcitTag) {
      return ["::", x.tag.toString(), ...x.extra.map(to_cirru_edn)];
    } else if (x.tag instanceof CalcitRecord) {
      return ["::", x.tag.name.toString(), ...x.extra.map(to_cirru_edn)];
    } else {
      throw new Error(`Unsupported tag for EDN: ${x.tag}`);
    }
  }
  if (x instanceof CalcitRef) {
    return ["atom", to_cirru_edn(x.value)];
  }
  console.error(x);
  throw new Error("Unexpected data to to-cirru-edn");
};

let recordFieldOrder = (a: [string, CirruEdnFormat], b: [string, CirruEdnFormat]) => {
  let a1_literal = isLiteral(a[1] as CalcitValue);
  let b1_literal = isLiteral(b[1] as CalcitValue);
  if (a1_literal && !b1_literal) {
    return -1;
  } else if (!a1_literal && b1_literal) {
    return 1;
  } else {
    return _$n_compare(a[0] as CalcitValue, b[0] as CalcitValue);
  }
};

/** makes sure we got string */
let extractFieldTag = (x: string) => {
  if (x[0] === ":") {
    return newTag(x.slice(1));
  } else {
    return newTag(x);
  }
};

export let extract_cirru_edn = (x: CirruEdnFormat, options: CalcitValue): CalcitValue => {
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
    // strict behavior as Rust semantics
    throw new Error(`unknown syntax for EDN: ${x}`);
  }
  if (x instanceof Array) {
    if (x.length === 0) {
      throw new Error("Cannot be empty form");
    }
    if (x[0] === "{}") {
      let result: Array<CalcitValue> = [];
      x.forEach((pair, idx) => {
        if (idx === 0) {
          return; // skip first `{}` symbol
        }
        if (pair instanceof Array) {
          if (pair[0] === ";") return;
          if (pair.length === 2) {
            result.push(extract_cirru_edn(pair[0], options), extract_cirru_edn(pair[1], options));
          } else {
            throw new Error(`Expected a pair, got: ${pair}`);
          }
        } else {
          throw new Error(`Expected pairs for map, got: ${pair}`);
        }
      });
      return new CalcitSliceMap(result);
    }
    if (x[0] === "%{}") {
      let name = x[1];
      if (typeof name != "string") {
        throw new Error(`Expected string for record name, got: ${name}`);
      }
      // put to entries first, sort and then...
      let entries: Array<[CalcitTag, CalcitValue]> = [];
      x.forEach((pair, idx) => {
        if (idx <= 1) {
          return; // skip %{} name
        }
        if (pair instanceof Array) {
          if (pair[0] === ";") return;
          if (pair.length === 2) {
            if (typeof pair[0] === "string") {
              entries.push([extractFieldTag(pair[0]), extract_cirru_edn(pair[1], options)]);
            } else {
              throw new Error(`Expected string as field, got: ${pair}`);
            }
          } else {
            throw new Error(`Expected pair of size 2, got: ${pair}`);
          }
        } else {
          throw new Error(`Expected pairs for reocrd, got: ${pair}`);
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

      if (options instanceof CalcitMap || options instanceof CalcitSliceMap) {
        let v = options.get(extractFieldTag(name));
        if (v != null && v instanceof CalcitRecord) {
          if (!deepEqual(v.fields, fields)) {
            throw new Error(`Fields mismatch for ${name}, expected ${fields}, got ${v.fields}`);
          }
          return new CalcitRecord(extractFieldTag(name), fields, values, v.klass);
        }
      }

      return new CalcitRecord(extractFieldTag(name), fields, values);
    }
    let notComment = (x: any) => {
      if (x instanceof Array && x[0] === ";") {
        return false;
      }
      return true;
    };
    if (x[0] === "[]") {
      return new CalcitSliceList(
        x
          .slice(1)
          .filter(notComment)
          .map((x) => extract_cirru_edn(x, options))
      );
    }
    if (x[0] === "#{}") {
      return new CalcitSet(
        x
          .slice(1)
          .filter(notComment)
          .map((x) => extract_cirru_edn(x, options))
      );
    }
    if (x[0] === "do" && x.length === 2) {
      return extract_cirru_edn(x[1], options);
    }
    if (x[0] === "quote") {
      if (x.length !== 2) {
        throw new Error(`quote expects 1 argument, got: ${x}`);
      }
      return new CalcitCirruQuote(x[1]);
    }
    if (x[0] === "::") {
      if (x.length < 2) {
        throw new Error(`tuple expects at least 1 value, got: ${x}`);
      }
      return new CalcitTuple(
        extract_cirru_edn(x[1], options),
        x
          .slice(2)
          .filter(notComment)
          .map((x) => extract_cirru_edn(x, options)),
        undefined
      );
    }
    if (x[0] === "atom") {
      if (x.length !== 2) {
        throw new Error(`atom expects 1 argument, got: ${x}`);
      }
      return atom(extract_cirru_edn(x[1], options));
    }
  }
  console.error(x);
  throw new Error(`Unexpected data from EDN: ${x}`);
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
