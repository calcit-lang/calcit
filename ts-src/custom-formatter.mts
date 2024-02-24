import { CalcitValue, is_literal } from "./js-primes.mjs";
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
    styleCode += `${kabab(key)}:${(o as any)[key]};`;
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
            for (let idx = 0; idx < obj.len(); idx++) {
              preview += " ";
              if (is_literal(obj.get(idx))) {
                preview += obj.get(idx).toString();
              } else {
                preview += "..";
                break;
              }
            }
            return div(
              {
                color: hsl(280, 80, 60, 0.4),
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
            for (let [k, v] of obj.pairs()) {
              preview += " ";
              if (is_literal(k) && is_literal(v)) {
                preview += `(${k.toString()} ${v.toString()})`;
              } else {
                preview += "..";
                break;
              }
            }
            return div({ color: hsl(280, 80, 60, 0.4) }, "{}", preview);
          }
          if (obj instanceof CalcitSet) {
            return div({ color: hsl(280, 80, 60, 0.4) }, obj.toString(true));
          }
          if (obj instanceof CalcitRecord) {
            let ret: any[] = div({ color: hsl(280, 80, 60, 0.4) }, `%{} ${obj.name} ...`);
            return ret;
          }
          if (obj instanceof CalcitTuple) {
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
            let has_collection = false;
            for (let idx = 0; idx < obj.len(); idx++) {
              if (!is_literal(obj.get(idx))) {
                has_collection = true;
                break;
              }
            }
            return obj.len() > 0 && has_collection;
          }
          if (obj instanceof CalcitMap || obj instanceof CalcitSliceMap) {
            let has_collection = false;
            for (let [k, v] of obj.pairs()) {
              if (!is_literal(k) || !is_literal(v)) {
                has_collection = true;
                break;
              }
            }
            return obj.len() > 0 && has_collection;
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
              let ka = pa[0].toString();
              let kb = pb[0].toString();
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
                  {
                    marginLeft: "8px",
                  },
                  td({ marginLeft: "8px", verticalAlign: "top" }, embedObject(k)),
                  td({ marginLeft: "8px" }, embedObject(v))
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
                  {
                    marginLeft: "8px",
                  },
                  td({ marginLeft: "8px", verticalAlign: "top" }, embedObject(obj.fields[idx])),
                  td({ marginLeft: "8px" }, embedObject(obj.values[idx]))
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
