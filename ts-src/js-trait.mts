import { CalcitTag, castTag, toString } from "./calcit-data.mjs";
import { CalcitValue } from "./js-primes.mjs";

// A realm can retain multiple runtime copies during dependency refresh. Keep
// one weak identity table, without branding or mutating host trait objects.
type TraitIdentities = { ids: WeakMap<object, number>; next: number };
const identityKey = Symbol.for("@calcit/procs/trait-identities");
const realm = globalThis as typeof globalThis & { [identityKey]?: TraitIdentities };
const identities = realm[identityKey] ??= { ids: new WeakMap<object, number>(), next: 0 };

/** Internal identity for hashing and ordering, never serialized or exposed as a Calcit value. */
export function traitIdentity(value: CalcitTrait): number {
  const identity = identities.ids.get(value);
  if (identity === undefined) throw new Error("Expected a nominal trait definition");
  return identity;
}

export class CalcitTrait {
  name: CalcitTag;
  methods: CalcitTag[];
  methodTypes: CalcitValue[];
  requires: CalcitTrait[];

  static [Symbol.hasInstance](value: unknown): boolean {
    return typeof value === "object" && value !== null && identities.ids.has(value);
  }

  constructor(name: CalcitValue, methods: CalcitValue[], methodTypes: CalcitValue[], requires: CalcitTrait[] = []) {
    this.name = castTag(name);
    this.methods = methods.map(castTag);
    this.methodTypes = methodTypes;
    this.requires = requires;
    identities.ids.set(this, ++identities.next);
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
