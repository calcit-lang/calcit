import { CalcitValue, isLiteral } from "./js-primes.mjs";
import { CalcitRef, CalcitSymbol, CalcitTag } from "./calcit-data.mjs";

import { CalcitRecord } from "./js-record.mjs";
import { CalcitMap, CalcitSliceMap } from "./js-map.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";
import { CalcitSet } from "./js-set.mjs";
import { CalcitTuple } from "./js-tuple.mjs";
import { CalcitCirruQuote } from "./js-cirru.mjs";

declare global {
  interface Window {
    devtoolsFormatters: {
      header: (obj: any, config: any) => any[];
      hasBody: (obj: any) => boolean;
      body: (obj: any, config: any) => any[];
    }[];
  }
}

let embedObject = (x: CalcitValue) => {
  if (x == null) {
    return null;
  }
  if (typeof x === "string") {
    return span({ whiteSpace: "pre-line", color: hsl(120, 70, 50), maxWidth: "100vw" }, `|${x}`);
  }
  return [
    "object",
    {
      object: x,
    },
  ];
};

/** camel case to kabab case */
let kabab = (s: string) => {
  return s.replace(/[A-Z]/g, (m) => "-" + m.toLowerCase());
};

/** returns {style: "..."} */
let styles = (o: any) => {
  let styleCode = "";
  let keys = Object.keys(o);
  for (let idx = 0; idx < keys.length; idx++) {
    let key = keys[idx];
    let value = (o as any)[key];
    if (value) {
      styleCode += `${kabab(key)}:${value};`;
    }
  }
  return {
    style: styleCode,
  };
};

let hsl = (/** 0~360 */ h: number, /** 0~100 */ s: number, /** 0~100 */ l: number, /** 0~1 */ a?: number) => {
  if (a != null) {
    return `hsla(${h}, ${s}%, ${l}%, ${a})`;
  }
  return `hsl(${h}, ${s}%, ${l}%)`;
};

/** create element */
let div = (style: any, ...children: any[]) => {
  return ["div", styles(style), ...children];
};
let span = (style: any, ...children: any[]) => {
  return ["span", styles(style), ...children];
};
let table = (style: any, ...children: any[]) => {
  return ["table", styles(style), ...children];
};
let tr = (style: any, ...children: any[]) => {
  return ["tr", styles(style), ...children];
};
let td = (style: any, ...children: any[]) => {
  return ["td", styles(style), ...children];
};

/** handle null value in nested data */
let saveString = (v: CalcitValue) => {
  if (typeof v === "string") {
    if (v.match(/[\s\"\n\t\,]/)) {
      return `"|${v}"`;
    } else {
      return `|${v}`;
    }
  } else if (v != null && v.toString) {
    return v.toString();
  } else {
    return "nil";
  }
};

export let load_console_formatter_$x_ = () => {
  if (typeof window === "object") {
    window["devtoolsFormatters"] = [
      {
        header: (obj, config) => {
          if (obj instanceof CalcitTag) {
            return div({ color: hsl(240, 80, 60) }, obj.toString());
          }
          if (obj instanceof CalcitSymbol) {
            return div({ color: hsl(240, 80, 60) }, obj.toString());
          }
          if (obj instanceof CalcitList || obj instanceof CalcitSliceList) {
            let preview = "";
            let hasCollection = false;
            for (let idx = 0; idx < obj.len(); idx++) {
              preview += " ";
              if (isLiteral(obj.get(idx))) {
                preview += saveString(obj.get(idx));
              } else {
                preview += "..";
                hasCollection = true;
                break;
              }
            }
            return div(
              {
                color: hasCollection ? hsl(280, 80, 60, 0.4) : null,
              },
              `[]`,
              span(
                {
                  fontSize: "8px",
                  verticalAlign: "middle",
                  color: hsl(280, 80, 80, 0.8),
                },
                `${obj.len()}`
              ),
              " ",
              preview
            );
          }
          if (obj instanceof CalcitMap || obj instanceof CalcitSliceMap) {
            let preview = "";
            let hasCollection = false;
            for (let [k, v] of obj.pairs()) {
              preview += " ";
              if (isLiteral(k) && isLiteral(v)) {
                preview += `(${saveString(k)} ${saveString(v)})`;
              } else {
                preview += "..";
                hasCollection = true;
                break;
              }
            }
            return div({ color: hasCollection ? hsl(280, 80, 60, 0.4) : undefined, maxWidth: "100%", whiteSpace: "normal" }, "{}", preview);
          }
          if (obj instanceof CalcitSet) {
            return div({ color: hsl(280, 80, 60, 0.4) }, obj.toString(true));
          }
          if (obj instanceof CalcitRecord) {
            let ret: any[] = div({ color: hsl(280, 80, 60, 0.4), maxWidth: "100%" }, `%{} ${obj.name} ...`);
            return ret;
          }
          if (obj instanceof CalcitTuple) {
            if (obj.klass) {
              let ret: any[] = div(
                {},
                div({ display: "inline-block", color: hsl(300, 100, 40) }, "%::"),
                div({ marginLeft: "6px", display: "inline-block" }, embedObject(obj.klass)),
                div({ marginLeft: "6px", display: "inline-block" }, embedObject(obj.tag))
              );
              for (let idx = 0; idx < obj.extra.length; idx++) {
                ret.push(div({ marginLeft: "6px", display: "inline-block" }, embedObject(obj.extra[idx])));
              }
              return ret;
            } else {
              let ret: any[] = div(
                {},
                div({ display: "inline-block", color: hsl(300, 100, 40) }, "::"),
                div({ marginLeft: "6px", display: "inline-block" }, embedObject(obj.tag))
              );
              for (let idx = 0; idx < obj.extra.length; idx++) {
                ret.push(div({ marginLeft: "6px", display: "inline-block" }, embedObject(obj.extra[idx])));
              }
              return ret;
            }
          }
          if (obj instanceof CalcitRef) {
            return div(
              {
                color: hsl(280, 80, 60),
              },
              `Ref ${obj.path}`,
              div({ color: hsl(280, 80, 60) }, div({ marginLeft: "8px" }, embedObject(obj.value)))
            );
          }
          if (obj instanceof CalcitCirruQuote) {
            return div(
              { color: hsl(280, 80, 60), display: "flex" },
              `CirruQuote`,
              div(
                { color: hsl(280, 80, 60), padding: "4px 4px", margin: "0 4px 2px", border: "1px solid hsl(0,70%,90%)", borderRadius: "4px" },
                obj.textForm().trim()
              )
            );
          }
          return null;
        },
        hasBody: (obj) => {
          if (obj instanceof CalcitList || obj instanceof CalcitSliceList) {
            let hasCollection = obj.nestedDataInChildren();
            return obj.len() > 0 && hasCollection;
          }
          if (obj instanceof CalcitMap || obj instanceof CalcitSliceMap) {
            let hasCollection = obj.nestedDataInChildren();
            return obj.len() > 0 && hasCollection;
          }
          if (obj instanceof CalcitSet) {
            return obj.len() > 0;
          }
          if (obj instanceof CalcitRecord) {
            return obj.fields.length > 0;
          }
          return false;
        },
        body: (obj, config) => {
          if (obj instanceof CalcitList || obj instanceof CalcitSliceList) {
            let flexMode = obj.len() > 40 ? "inline-flex" : "flex";
            return div(
              { color: hsl(280, 80, 60), borderLeft: "1px solid #eee" },
              ...(obj.toArray().map((x, idx) => {
                return div(
                  { marginLeft: "8px", display: flexMode, paddingRight: "16px" },
                  span(
                    {
                      fontFamily: "monospace",
                      marginRight: "8px",
                      color: hsl(280, 80, 90),
                      flexShrink: 0,
                      fontSize: "10px",
                      whiteSpace: "nowrap",
                    },
                    idx
                  ),
                  embedObject(x)
                );
              }) as any[])
            );
          }
          if (obj instanceof CalcitSet) {
            let ret: any[] = div({ color: hsl(280, 80, 60), borderLeft: "1px solid #eee" });
            let values = obj.values();
            for (let idx = 0; idx < values.length; idx++) {
              let x = values[idx];
              ret.push(div({ marginLeft: "8px", display: "inline-block" }, embedObject(x)));
            }
            return ret;
          }
          if (obj instanceof CalcitMap || obj instanceof CalcitSliceMap) {
            let ret: any[] = table({ color: hsl(280, 80, 60), borderLeft: "1px solid #eee" });
            let pairs = obj.pairs();
            pairs.sort((pa, pb) => {
              let ka = saveString(pa[0]);
              let kb = saveString(pb[0]);
              if (ka < kb) {
                return -1;
              } else if (ka > kb) {
                return 1;
              } else {
                return 0;
              }
            });
            for (let idx = 0; idx < pairs.length; idx++) {
              let [k, v] = pairs[idx];
              ret.push(
                tr(
                  {},
                  td({ paddingLeft: "8px", verticalAlign: "top", whiteSpace: "nowrap", minWidth: "40px" }, embedObject(k)),
                  td({ paddingLeft: "8px" }, embedObject(v))
                )
              );
            }
            return ret;
          }
          if (obj instanceof CalcitRecord) {
            let ret: any[] = table({ color: hsl(280, 80, 60), borderLeft: "1px solid #eee" });
            for (let idx = 0; idx < obj.fields.length; idx++) {
              ret.push(
                tr(
                  {},
                  td({ paddingLeft: "8px", verticalAlign: "top", whiteSpace: "pre", minWidth: "40px" }, embedObject(obj.fields[idx])),
                  td({ paddingLeft: "8px" }, embedObject(obj.values[idx]))
                )
              );
            }
            return ret;
          }

          return null;
        },
      },
    ];
  }
};
