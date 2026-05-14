import { CalcitValue } from "./js-primes.mjs";
import { CalcitList, CalcitSliceList } from "./js-list.mjs";

// === CalcitBufList — mutable append-only list ===
// Wrapper around a plain JS Array for O(1) push.
// Use &buf-list:new / &buf-list:push / &buf-list:to-list in Calcit code.
export class CalcitBufList {
  buf: CalcitValue[];
  constructor(buf?: CalcitValue[]) {
    this.buf = buf ?? [];
  }
}

export function _$n_buf_list_$o_new(): CalcitBufList {
  return new CalcitBufList();
}

export function _$n_buf_list_$o_push(buf: CalcitValue, item: CalcitValue): CalcitBufList {
  if (!(buf instanceof CalcitBufList)) throw new Error(`&buf-list:push expected a buf-list, got ${buf}`);
  buf.buf.push(item);
  return buf;
}

export function _$n_buf_list_$o_concat(buf: CalcitValue, xs: CalcitValue): CalcitBufList {
  if (!(buf instanceof CalcitBufList)) throw new Error(`&buf-list:concat expected a buf-list, got ${buf}`);
  if (xs instanceof CalcitSliceList || xs instanceof CalcitList) {
    const gen = xs.items();
    let next = gen.next();
    while (!next.done) {
      buf.buf.push(next.value);
      next = gen.next();
    }
  } else {
    throw new Error(`&buf-list:concat expected a list, got ${xs}`);
  }
  return buf;
}

export function _$n_buf_list_$o_to_list(buf: CalcitValue): CalcitSliceList {
  if (!(buf instanceof CalcitBufList)) throw new Error(`&buf-list:to-list expected a buf-list, got ${buf}`);
  return new CalcitSliceList([...buf.buf]);
}

export function _$n_buf_list_$o_count(buf: CalcitValue): number {
  if (!(buf instanceof CalcitBufList)) throw new Error(`&buf-list:count expected a buf-list, got ${buf}`);
  return buf.buf.length;
}
