import { overwriteComparator, initTernaryTreeMap } from "@calcit/ternary-tree";
import { CirruWriterNode, writeCirruCode, writeCirruOneLiner } from "@cirru/writer.ts";

import { CalcitValue, isLiteral, _$n_compare } from "./js-primes.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";
import { CalcitStructValue } from "./js-struct-value.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitSet } from "./js-set.mjs";
import { CalcitTag, CalcitSymbol, CalcitRecur, newTag, compareTagNames } from "./calcit-data.mjs";
import { CalcitEnumValue } from "./js-enum-value.mjs";
import { CalcitEnumDef } from "./js-enum-def.mjs";
import { CalcitImpl } from "./js-impl.mjs";
import { CalcitRef } from "./js-ref.mjs";
import { deepEqual } from "@calcit/ternary-tree/lib/utils.mjs";
import { atom } from "./js-ref.mjs";
import { TypedEdnSetView } from "./typed-edn.mjs";

type CirruEdnFormat = string | CirruEdnFormat[];

export class CalcitCirruQuote {
  value: CirruWriterNode[];
  constructor(value: CirruWriterNode[]) {
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
        return new CalcitCirruQuote(this.value[idx] as CirruWriterNode[]);
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

export let format_cirru_one_liner = (data: CalcitCirruQuote | CalcitList): string => {
  let chunk: CirruWriterNode;

  if (data instanceof CalcitCirruQuote) {
    chunk = data.value;
  } else {
    chunk = toWriterNode(data);
  }

  if (!Array.isArray(chunk)) {
    throw new Error("Expected data of list");
  }

  return writeCirruOneLiner(chunk);
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
  if (x instanceof CalcitStructValue) {
    let buffer: [string, CirruEdnFormat][] = [];
    for (let idx = 0; idx < x.fields.length; idx++) {
      buffer.push([x.fields[idx].toString(), to_cirru_edn(x.values[idx])]);
    }
    // placed literals first
    buffer.sort(recordFieldOrder);
    (buffer as any[]).unshift(new CalcitSymbol(x.name.value).toString());
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
  if (x instanceof CalcitEnumValue) {
    if (x.tag instanceof CalcitSymbol && x.tag.value === "quote") {
      // turn `x.snd` with CalcitList into raw Cirru nodes, which is in plain Array
      return ["quote", toWriterNode(x.get(1) as any)] as CirruEdnFormat;
    } else if (x.enumPrototype != null) {
      let enumTag = new CalcitSymbol(unwrap_enum_prototype_local(x.enumPrototype).name.value).toString();
      if (x.tag instanceof CalcitTag) {
        return ["%::", enumTag, new CalcitSymbol(x.tag.value).toString(), ...x.extra.map(to_cirru_edn)];
      } else if (x.tag instanceof CalcitStructValue) {
        return ["%::", enumTag, new CalcitSymbol(x.tag.name.value).toString(), ...x.extra.map(to_cirru_edn)];
      } else if (x.tag instanceof CalcitImpl) {
        return ["%::", enumTag, new CalcitSymbol(x.tag.name.value).toString(), ...x.extra.map(to_cirru_edn)];
      } else {
        throw new Error(`Unsupported tag for EDN: ${x.tag}`);
      }
    } else if (x.tag instanceof CalcitTag) {
      return ["::", new CalcitSymbol(x.tag.value).toString(), ...x.extra.map(to_cirru_edn)];
    } else if (x.tag instanceof CalcitStructValue) {
      return ["::", new CalcitSymbol(x.tag.name.value).toString(), ...x.extra.map(to_cirru_edn)];
    } else if (x.tag instanceof CalcitImpl) {
      return ["::", new CalcitSymbol(x.tag.name.value).toString(), ...x.extra.map(to_cirru_edn)];
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
  if (x[0] === ":" || x[0] === "'") {
    return newTag(x.slice(1));
  } else {
    return newTag(x);
  }
};

let extractEnumTag = (x: CirruEdnFormat, options: CalcitValue, preserveSourceEntries: boolean): CalcitValue => {
  const parsedTag = extract_cirru_edn_inner(x, options, preserveSourceEntries);
  return parsedTag instanceof CalcitSymbol ? newTag(parsedTag.value) : parsedTag;
};

let resolveEnumPrototype = (enumName: string, options: CalcitValue) => {
  if (options instanceof CalcitMap || options instanceof CalcitSliceMap) {
    let value = options.get(extractFieldTag(enumName));
    if (value instanceof CalcitEnumDef) {
      return value;
    }
    if (value instanceof CalcitStructValue) {
      throw new Error(`Enum ${enumName} uses a legacy struct prototype; provide an EnumDef produced by defenum`);
    }
    if (value != null) {
      throw new Error(`Expected enum prototype for ${enumName}, got: ${value}`);
    }
  }
  return null;
};

// local helper to inspect an EnumDef's variant prototype
const unwrap_enum_prototype_local = (enumPrototype: CalcitValue): CalcitStructValue => {
  if (enumPrototype instanceof CalcitEnumDef) {
    return enumPrototype.prototype;
  }
  throw new Error(`expected an EnumDef produced by defenum`);
};

const tag_to_string = (tag: CalcitValue): string => {
  if (tag instanceof CalcitTag) return tag.toString();
  if (tag instanceof CalcitStructValue) return tag.name.toString();
  throw new Error(`Unsupported tag for EDN: ${tag}`);
};

const extract_cirru_edn_inner = (x: CirruEdnFormat, options: CalcitValue, preserveSourceEntries: boolean): any => {
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
      const sourceKeys: CirruEdnFormat[] = [];
      x.forEach((pair, idx) => {
        if (idx === 0) {
          return; // skip first `{}` symbol
        }
        if (pair instanceof Array) {
          if (pair[0] === ";") return;
          if (pair.length === 2) {
            const key = extract_cirru_edn_inner(pair[0], options, preserveSourceEntries);
            const value = extract_cirru_edn_inner(pair[1], options, preserveSourceEntries);
            const existingIdx = preserveSourceEntries ? sourceKeys.findIndex((sourceKey) => deepEqual(sourceKey, pair[0])) : -1;
            if (existingIdx >= 0) {
              result[(existingIdx << 1) + 1] = value;
            } else {
              if (preserveSourceEntries) sourceKeys.push(pair[0]);
              result.push(key, value);
            }
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
        throw new Error(`Expected string for struct name, got: ${name}`);
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
              entries.push([
                extractFieldTag(pair[0]),
                extract_cirru_edn_inner(pair[1], options, preserveSourceEntries),
              ]);
            } else {
              throw new Error(`Expected string as field, got: ${pair}`);
            }
          } else {
            throw new Error(`Expected pair of size 2, got: ${pair}`);
          }
        } else {
          throw new Error(`Expected field pairs for struct, got: ${pair}`);
        }
      });
      entries.sort((a, b) => compareTagNames(a[0], b[0]));
      let fields: Array<CalcitTag> = [];
      let values: Array<CalcitValue> = [];

      for (let idx = 0; idx < entries.length; idx++) {
        fields.push(entries[idx][0]);
        values.push(entries[idx][1]);
      }

      if (options instanceof CalcitMap || options instanceof CalcitSliceMap) {
        let v = options.get(extractFieldTag(name));
        if (v != null && v instanceof CalcitStructValue) {
          if (deepEqual(v.fields, fields)) {
            return new CalcitStructValue(extractFieldTag(name), fields, values, v.structRef);
          }
        }
      }

      return new CalcitStructValue(extractFieldTag(name), fields, values);
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
          .map((x) => extract_cirru_edn_inner(x, options, preserveSourceEntries))
      );
    }
    if (x[0] === "#{}") {
      const sourceItems = x.slice(1).filter(notComment);
      const uniqueSourceItems = preserveSourceEntries
        ? sourceItems.filter((item, idx) => !sourceItems.slice(0, idx).some((existing) => deepEqual(existing, item)))
        : sourceItems;
      const items = uniqueSourceItems.map((x) => extract_cirru_edn_inner(x, options, preserveSourceEntries));
      return preserveSourceEntries ? new TypedEdnSetView(items) : new CalcitSet(items);
    }
    if (x[0] === "do" && x.length === 2) {
      return extract_cirru_edn_inner(x[1], options, preserveSourceEntries);
    }
    if (x[0] === "quote") {
      if (x.length !== 2) {
        throw new Error(`quote expects 1 argument, got: ${x}`);
      }
      return new CalcitCirruQuote(x[1] as CirruWriterNode[]);
    }
    if (x[0] === "::") {
      if (x.length < 2) {
        throw new Error(`anonymous enum expects at least 1 value, got: ${x}`);
      }
      return new CalcitEnumValue(
        extractEnumTag(x[1], options, preserveSourceEntries),
        x
          .slice(2)
          .filter(notComment)
          .map((x) => extract_cirru_edn_inner(x, options, preserveSourceEntries))
      );
    }
    if (x[0] === "%::") {
      if (x.length < 3) {
        throw new Error(`%:: expects at least 2 values, got: ${x}`);
      }
      let enumName = x[1];
      if (typeof enumName !== "string") {
        throw new Error(`Expected string for enum name, got: ${enumName}`);
      }
      let enumPrototype = resolveEnumPrototype(enumName, options);
      return new CalcitEnumValue(
        extractEnumTag(x[2], options, preserveSourceEntries),
        x
          .slice(3)
          .filter(notComment)
          .map((x) => extract_cirru_edn_inner(x, options, preserveSourceEntries)),
        enumPrototype
      );
    }
    if (x[0] === "atom") {
      if (x.length !== 2) {
        throw new Error(`atom expects 1 argument, got: ${x}`);
      }
      return atom(extract_cirru_edn_inner(x[1], options, preserveSourceEntries));
    }
  }
  console.error(x);
  throw new Error(`Unexpected data from EDN: ${x}`);
};

export let extract_cirru_edn = (x: CirruEdnFormat, options: CalcitValue): CalcitValue => {
  return extract_cirru_edn_inner(x, options, false) as CalcitValue;
};

export let extract_cirru_edn_for_typed = (x: CirruEdnFormat, options: CalcitValue): CalcitValue | TypedEdnSetView => {
  return extract_cirru_edn_inner(x, options, true) as CalcitValue | TypedEdnSetView;
};

export let format_cirru_edn = (data: CalcitValue, useInline: boolean = true): string => {
  if (data == null) {
    return "\ndo nil" + "\n";
  }
  if (typeof data === "string") {
    let quoted = writeCirruCode([[to_cirru_edn(data)]], { useInline: useInline }).trim();
    return "\ndo " + quoted + "\n";
  }
  if (typeof data === "boolean") {
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
  if (x instanceof CalcitStructValue) return x;
  if (x instanceof CalcitRecur) return x;
  if (x instanceof CalcitRef) return x;
  if (x instanceof CalcitTag) return x;
  if (x instanceof CalcitSymbol) return x;
  if (x instanceof CalcitEnumValue) return x;

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
