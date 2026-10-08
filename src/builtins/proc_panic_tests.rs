//! Randomized check that no builtin proc panics on arbitrary `Calcit` input.
//! A proc may reject an input with `CalcitErr`; it must never unwind or abort.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use strum::IntoEnumIterator;

use super::handle_proc;
use crate::calcit::{Calcit, CalcitList, CalcitProc};
use crate::call_stack::CallStackList;

/// Procs with effects outside the process value space (exit, sleep, stdin, files,
/// stdout, global counters) are skipped; their failure classes are still declared.
fn skipped(proc: CalcitProc) -> bool {
  use CalcitProc::*;
  matches!(
    proc,
    Quit
      | NativeWaitMs
      | NativeReadStdinText
      | WriteFile
      | NativeFsWriteText
      | NativeDisplayStack
      | NativeResetGenSymIndex
      | RegisterCalcitBuiltinImpls
  )
}

/// Procs whose numeric arguments size an allocation. Huge sizes exhaust memory
/// instead of failing (#1852), so these only receive numbers below 2^31 here.
fn sizes_allocation(proc: CalcitProc) -> bool {
  use CalcitProc::*;
  matches!(proc, Range | NativeListRange)
}

/// Deterministic xorshift generator so a failure reproduces from its seed.
struct Rng(u64);

impl Rng {
  fn next(&mut self) -> u64 {
    let mut x = self.0;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    self.0 = x;
    x
  }

  fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
    &items[(self.next() % items.len() as u64) as usize]
  }
}

fn value_pool() -> Vec<Calcit> {
  let numbers = [
    0.0,
    -0.0,
    1.0,
    -1.0,
    2.0,
    1.5,
    -2.5,
    255.0,
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    2147483648.0,
    -2147483649.0,
    4294967296.0,
    9007199254740993.0,
    1e300,
    -1e300,
  ];
  let mut pool: Vec<Calcit> = numbers.iter().map(|n| Calcit::Number(*n)).collect();
  for s in ["", "a", "a,b", "中文", "😀", "1.5", "  ", "\u{0}"] {
    pool.push(Calcit::Str(Arc::from(s)));
  }
  pool.extend([
    Calcit::Nil,
    Calcit::Unit,
    Calcit::Bool(true),
    Calcit::Bool(false),
    Calcit::tag("a"),
    Calcit::tag("int32"),
    Calcit::Buffer(Arc::from(vec![0u8, 255])),
    Calcit::from(CalcitList::from(&[] as &[Calcit])),
    Calcit::from(CalcitList::from(&[Calcit::Number(1.0), Calcit::Number(2.0)] as &[Calcit])),
    Calcit::from(CalcitList::from(
      &[Calcit::from(CalcitList::from(&[Calcit::Nil] as &[Calcit]))] as &[Calcit]
    )),
    Calcit::Map(rpds::HashTrieMap::new_sync()),
    Calcit::Map(rpds::HashTrieMap::new_sync().insert(Calcit::tag("a"), Calcit::Number(1.0))),
    Calcit::Set(rpds::HashTrieSet::new_sync()),
    Calcit::Set(rpds::HashTrieSet::new_sync().insert(Calcit::Number(1.0))),
  ]);
  pool
}

#[test]
fn builtin_procs_do_not_panic_on_arbitrary_input() {
  let pool = value_pool();
  let call_stack = CallStackList::default();
  // Set CALCIT_PROC_FUZZ_TRACE to print each call first, which locates an abort
  // (stack or memory exhaustion) that `catch_unwind` cannot report.
  let trace = std::env::var("CALCIT_PROC_FUZZ_TRACE").is_ok();
  let mut panics: Vec<String> = vec![];
  for (proc_index, proc) in CalcitProc::iter().enumerate() {
    if skipped(proc) {
      continue;
    }
    let seed = 0x9e37_79b9_7f4a_7c15u64 ^ (proc_index as u64 + 1);
    let mut rng = Rng(seed);
    for round in 0..500 {
      let arg_count = (rng.next() % 5) as usize;
      let args: Vec<Calcit> = (0..arg_count)
        .map(|_| match rng.pick(&pool) {
          Calcit::Number(n) if sizes_allocation(proc) && n.abs() >= 2147483648.0 => Calcit::Number(2.0),
          value => value.to_owned(),
        })
        .collect();
      let rendered = || args.iter().map(|a| format!("{a}")).collect::<Vec<_>>().join(" ");
      if trace {
        eprintln!("{proc} {round} {}", rendered());
      }
      let outcome = catch_unwind(AssertUnwindSafe(|| handle_proc(proc, &args, &call_stack)));
      if outcome.is_err() {
        let rendered = rendered();
        panics.push(format!("{proc} (seed {seed:#x}, round {round}): {rendered}"));
        break;
      }
    }
  }

  assert!(panics.is_empty(), "builtin procs panicked:\n  {}", panics.join("\n  "));
}
