import { CrDataRef, CrDataValue, CrDataSymbol, CrDataKeyword, CrDataList, CrDataMap, CrDataRecord, CrDataSet } from "./calcit-data";
import { toPairs } from "@calcit/ternary-tree";

declare global {
  interface Window {
    devtoolsFormatters: {
      header: (obj: any, config: any) => any[];
      hasBody: (obj: any) => boolean;
      body: (obj: any, config: any) => any[];
    }[];
  }
}

let embedObject = (x: CrDataValue) => {
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

export let load_console_formatter_BANG_ = () => {
  if (typeof window === "object") {
    window["devtoolsFormatters"] = [
      {
        header: (obj, config) => {
          if (obj instanceof CrDataKeyword) {
            return ["div", { style: "color: hsl(240, 80%, 60%)" }, obj.toString()];
          }
          if (obj instanceof CrDataSymbol) {
            return ["div", { style: "color: hsl(340, 80%, 60%)" }, obj.toString()];
          }
          if (obj instanceof CrDataList) {
            return [
              "div",
              { style: "color: hsl(280, 80%, 60%, 0.4)" },
              obj.toString(true),
              ["span", { style: "font-size: 80%; vertical-align: 0.7em; color: hsl(280, 80%, 60%, 0.8)" }, `${obj.len()}`],
            ];
          }
          if (obj instanceof CrDataMap) {
            return ["div", { style: "color: hsl(280, 80%, 60%, 0.4)" }, obj.toString(true)];
          }
          if (obj instanceof CrDataSet) {
            return ["div", { style: "color: hsl(280, 80%, 60%, 0.4)" }, obj.toString()];
          }
          if (obj instanceof CrDataRecord) {
            let ret: any[] = ["div", { style: "color: hsl(280, 80%, 60%)" }, `%{} ${obj.name}`];
            for (let idx in obj.fields) {
              ret.push([
                "div",
                { style: "margin-left: 8px;" },
                ["div", { style: "margin-left: 8px; display: inline-block;" }, obj.fields[idx]],
                ["div", { style: "margin-left: 8px; display: inline-block;" }, embedObject(obj.values[idx])],
              ]);
            }
            return ret;
          }
          if (obj instanceof CrDataRef) {
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
          if (obj instanceof CrDataList) {
            return obj.len() > 0;
          }
          if (obj instanceof CrDataMap) {
            return obj.len() > 0;
          }
          if (obj instanceof CrDataSet) {
            return obj.len() > 0;
          }
          return false;
        },
        body: (obj, config) => {
          if (obj instanceof CrDataList) {
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
          if (obj instanceof CrDataSet) {
            let ret: any[] = ["div", { style: "color: hsl(280, 80%, 60%)" }];
            for (let x of obj.value.values()) {
              ret.push(["div", { style: "margin-left: 8px; display: inline-block;" }, embedObject(x)]);
            }
            return ret;
          }
          if (obj instanceof CrDataMap) {
            let ret: any[] = ["div", { style: "color: hsl(280, 80%, 60%)" }];
            obj.turnMap();
            for (let [k, v] of toPairs(obj.value)) {
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
