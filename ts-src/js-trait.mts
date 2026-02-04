import { CalcitTag, castTag, toString } from "./calcit-data.mjs";
import { CalcitValue } from "./js-primes.mjs";

export class CalcitTrait {
  name: CalcitTag;
  methods: CalcitTag[];

  constructor(name: CalcitValue, methods: CalcitValue[]) {
    this.name = castTag(name);
    this.methods = methods.map(castTag);
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
