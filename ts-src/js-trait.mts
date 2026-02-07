import { CalcitTag, castTag, toString } from "./calcit-data.mjs";
import { CalcitValue } from "./js-primes.mjs";

export class CalcitTrait {
  name: CalcitTag;
  methods: CalcitTag[];
  methodTypes: CalcitValue[];

  constructor(name: CalcitValue, methods: CalcitValue[], methodTypes: CalcitValue[]) {
    this.name = castTag(name);
    this.methods = methods.map(castTag);
    this.methodTypes = methodTypes;
  }

  toString(disableJsDataWarning: boolean = false): string {
    const parts: string[] = ["(trait ", this.name.toString()];
    for (let i = 0; i < this.methods.length; i++) {
      parts.push(" ", this.methods[i].toString());
    }
    parts.push(")");
    return parts.join("");
  }
}
