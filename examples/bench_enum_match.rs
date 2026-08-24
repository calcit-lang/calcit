use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use calcit::{Calcit, CalcitEnumDef, CalcitList, CalcitStructDef, CalcitStructValue};
use cirru_edn::EdnTag;

const ITERATIONS: usize = 5_000_000;
const VARIANT_COUNT: usize = 16;

fn main() {
  let fields: Vec<EdnTag> = (0..VARIANT_COUNT).map(|idx| EdnTag::new(format!("variant-{idx:02}"))).collect();
  let values = vec![Calcit::from(CalcitList::Vector(vec![])); fields.len()];
  let enum_def = CalcitEnumDef::from_struct(CalcitStructValue {
    struct_ref: Arc::new(CalcitStructDef::from_fields(EdnTag::new("DispatchState"), fields)),
    values: Arc::new(values),
  })
  .expect("valid benchmark enum");
  let target = enum_def.variants().last().expect("last variant").tag.clone();

  run("linear-tag-scan", || {
    enum_def
      .variants()
      .iter()
      .position(|variant| variant.tag == target)
      .expect("target variant")
  });
  run("indexed-branch-slot", || enum_def.variant_index(&target).expect("target variant"));
}

fn run(label: &str, mut dispatch: impl FnMut() -> usize) {
  let mut checksum = 0usize;
  for _ in 0..100_000 {
    checksum = checksum.wrapping_add(black_box(dispatch()));
  }
  let started = Instant::now();
  for _ in 0..ITERATIONS {
    checksum = checksum.wrapping_add(black_box(dispatch()));
  }
  println!("{label}: {:.2}ms checksum={checksum}", started.elapsed().as_secs_f64() * 1000.0);
}
