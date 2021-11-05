import { CalcitValue } from "./js-primes";
import { CalcitRef, CalcitSymbol, CalcitKeyword } from "./calcit-data";
import { toPairs } from "@calcit/ternary-tree";

import { CalcitRecord } from "./js-record";
import { CalcitMap, CalcitSliceMap } from "./js-map";
import { CalcitList, CalcitSliceList } from "./js-list";
import { CalcitSet } from "./js-set";
import { CalcitTuple } from "./js-tuple";

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

export let load_console_formatter_$x_ = () => {
  if (typeof window === "object") {
    window["devtoolsFormatters"] = [
      {
        header: (obj, config) => {
          if (obj instanceof CalcitKeyword) {
            return ["div", { style: "color: hsl(240, 80%, 60%)" }, obj.toString()];
          }
          if (obj instanceof CalcitSymbol) {
            return ["div", { style: "color: hsl(340, 80%, 60%)" }, obj.toString()];
          }
          if (obj instanceof CalcitList || obj instanceof CalcitSliceList) {
            return [
              "div",
              { style: "color: hsl(280, 80%, 60%, 0.4)" },
              obj.toString(true),
              ["span", { style: "font-size: 80%; vertical-align: 0.7em; color: hsl(280, 80%, 60%, 0.8)" }, `${obj.len()}`],
            ];
          }
          if (obj instanceof CalcitMap || obj instanceof CalcitSliceMap) {
            return ["div", { style: "color: hsl(280, 80%, 60%, 0.4)" }, obj.toString(true)];
          }
          if (obj instanceof CalcitSet) {
            return ["div", { style: "color: hsl(280, 80%, 60%, 0.4)" }, obj.toString()];
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
          if (obj instanceof CalcitRef) {
            return [
              "div",
              { style: "color: hsl(280, 80%, 60%)" },
              `Ref ${obj.path}`,
              ["div", { style: "color: hsl(280, 80%, 60%)" }, ["div", { style: "margin-left: 8px;" }, embedObject(obj.value)]],
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
            return ["div", { style: "color: hsl(280, 80%, 60%)" }].concat(
              obj.toArray().map((x, idx) => {
                return [
                  "div",
                  { style: "margin-left: 8px; display: flex;" },
                  ["span", { style: "font-family: monospace; margin-right: 8px; color: hsl(280,80%,90%); flex-shrink: 0;" }, idx],
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
          if (obj instanceof CalcitTuple) {
            let ret: any[] = ["div", { style: "color: hsl(200, 90%, 60%)" }];
            ret.push(["div", { style: "margin-left: 8px; display: inline-block;" }, embedObject(obj.fst)]);
            ret.push(["div", { style: "margin-left: 8px; display: inline-block;" }, embedObject(obj.snd)]);
            return ret;
          }
          if (obj instanceof CalcitMap || obj instanceof CalcitSliceMap) {
            let ret: any[] = ["div", { style: "color: hsl(280, 80%, 60%)" }];
            let pairs = obj.pairs();
            for (let [k, v] of pairs) {
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
