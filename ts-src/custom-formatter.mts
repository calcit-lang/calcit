import { CalcitValue } from "./js-primes.mjs";
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

let shortPreview = (x: string) => {
  if (x.length > 102) {
    return x.substring(0, 100) + "...";
  }
  return x;
};

export let load_console_formatter_$x_ = () => {
  if (typeof window === "object") {
    window["devtoolsFormatters"] = [
      {
        header: (obj, config) => {
          if (obj instanceof CalcitTag) {
            return ["div", { style: "color: hsl(240, 80%, 60%)" }, obj.toString()];
          }
          if (obj instanceof CalcitSymbol) {
            return ["div", { style: "color: hsl(340, 80%, 60%)" }, obj.toString()];
          }
          if (obj instanceof CalcitList || obj instanceof CalcitSliceList) {
            return [
              "div",
              { style: "color: hsl(280, 80%, 60%, 0.4)" },
              shortPreview(obj.toString(true, true)),
              ["span", { style: "font-size: 80%; vertical-align: 0.7em; color: hsl(280, 80%, 60%, 0.8)" }, `${obj.len()}`],
            ];
          }
          if (obj instanceof CalcitMap || obj instanceof CalcitSliceMap) {
            return ["div", { style: "color: hsl(280, 80%, 60%, 0.4)" }, shortPreview(obj.toString(true, true))];
          }
          if (obj instanceof CalcitSet) {
            return ["div", { style: "color: hsl(280, 80%, 60%, 0.4)" }, obj.toString(true)];
          }
          if (obj instanceof CalcitRecord) {
            let ret: any[] = ["div", { style: "color: hsl(280, 80%, 60%)" }, `%{} ${obj.name}`];
            for (let idx = 0; idx < obj.fields.length; idx++) {
              ret.push([
                "div",
                { style: "margin-left: 8px;" },
                ["div", { style: "margin-left: 8px; display: inline-block;" }, embedObject(obj.fields[idx])],
                ["div", { style: "margin-left: 8px; display: inline-block;" }, embedObject(obj.values[idx])],
              ]);
            }
            return ret;
          }
          if (obj instanceof CalcitTuple) {
            let ret: any[] = ["div", {}];
            ret.push(["div", { style: "display: inline-block; color: hsl(300, 100%, 40%); " }, "::"]);
            ret.push(["div", { style: "margin-left: 6px; display: inline-block;" }, embedObject(obj.tag)]);
            for (let idx = 0; idx < obj.extra.length; idx++) {
              ret.push(["div", { style: "margin-left: 6px; display: inline-block;" }, embedObject(obj.extra[idx])]);
            }
            return ret;
          }
          if (obj instanceof CalcitRef) {
            return [
              "div",
              { style: "color: hsl(280, 80%, 60%)" },
              `Ref ${obj.path}`,
              ["div", { style: "color: hsl(280, 80%, 60%)" }, ["div", { style: "margin-left: 8px;" }, embedObject(obj.value)]],
            ];
          }
          if (obj instanceof CalcitCirruQuote) {
            return [
              "div",
              { style: "color: hsl(240, 80%, 60%); display: flex;" },
              `CirruQuote`,
              [
                "div",
                { style: "color: hsl(280, 80%, 60%); padding: 4px 4px; margin: 0 4px 2px; border: 1px solid hsl(0,70%,90%); border-radius: 4px;" },
                obj.textForm().trim(),
              ],
            ];
          }
          return null;
        },
        hasBody: (obj) => {
          if (obj instanceof CalcitList || obj instanceof CalcitSliceList) {
            return obj.len() > 0;
          }
          if (obj instanceof CalcitMap || obj instanceof CalcitSliceMap) {
            return obj.len() > 0;
          }
          if (obj instanceof CalcitSet) {
            return obj.len() > 0;
          }
          return false;
        },
        body: (obj, config) => {
          if (obj instanceof CalcitList || obj instanceof CalcitSliceList) {
            let flexMode = obj.len() > 40 ? "inline-flex" : "flex";
            return ["div", { style: "color: hsl(280, 80%, 60%)" }].concat(
              obj.toArray().map((x, idx) => {
                return [
                  "div",
                  { style: `margin-left: 8px; display: ${flexMode}; padding-right: 16px;` },
                  ["span", { style: "font-family: monospace; margin-right: 8px; color: hsl(280,80%,85%); flex-shrink: 0; font-size: 10px;" }, idx],
                  embedObject(x),
                ];
              }) as any[]
            );
          }
          if (obj instanceof CalcitSet) {
            let ret: any[] = ["div", { style: "color: hsl(280, 80%, 60%)" }];
            let values = obj.values();
            for (let idx = 0; idx < values.length; idx++) {
              let x = values[idx];
              ret.push(["div", { style: "margin-left: 8px; display: inline-block;" }, embedObject(x)]);
            }
            return ret;
          }
          if (obj instanceof CalcitMap || obj instanceof CalcitSliceMap) {
            let ret: any[] = ["div", { style: "color: hsl(280, 80%, 60%)" }];
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
              ret.push([
                "div",
                { style: "margin-left: 8px; display: flex;" },
                ["div", { style: "margin-left: 8px; flex-shrink: 0; display: inline-block;" }, embedObject(k)],
                ["div", { style: "margin-left: 8px; display: inline-block;" }, embedObject(v)],
              ]);
            }
            return ret;
          }

          return null;
        },
      },
    ];
  }
};
