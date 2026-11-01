import type { CalcitValue } from "./js-primes.mjs";

/// Runtime value of a `defunion`. Union values carry no wrapper, so this value
/// only names the union and keeps its member type forms for display.
export class CalcitUnionDef {
  name: string;
  members: CalcitValue[];
  cachedHash: number;

  constructor(name: string, members: CalcitValue[]) {
    this.name = name;
    this.members = members;
    this.cachedHash = null;
  }

  toString(): string {
    return `(%union-def '${this.name} ${this.members.map((member) => String(member)).join(" ")})`;
  }
}
