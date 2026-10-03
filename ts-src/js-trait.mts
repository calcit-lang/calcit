import { CalcitTag, castTag, toString } from "./calcit-data.mjs";
import { CalcitValue } from "./js-primes.mjs";

export class CalcitTrait {
  name: CalcitTag;
  methods: CalcitTag[];
  methodTypes: CalcitValue[];
  requires: CalcitTrait[];

  constructor(name: CalcitValue, methods: CalcitValue[], methodTypes: CalcitValue[], requires: CalcitTrait[] = []) {
    this.name = castTag(name);
    this.methods = methods.map(castTag);
    this.methodTypes = methodTypes;
    this.requires = requires;
  }

  /** This trait and every transitively required trait, each origin once. */
  reachable(): CalcitTrait[] {
    const output: CalcitTrait[] = [];
    const visit = (current: CalcitTrait, active: CalcitTrait[]) => {
      if (active.includes(current)) {
        throw new Error(`trait requires cycle: ${[...active, current].map((item) => item.name.toString()).join(" -> ")}`);
      }
      if (output.includes(current)) return;
      for (const required of current.requires) visit(required, [...active, current]);
      output.push(current);
    };
    visit(this, []);
    return output;
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
