#[macro_use]
extern crate lazy_static;

mod data;
mod primes;
mod program;
mod snapshot;

use cirru_edn::parse_cirru_edn;
use primes::CalcitData::*;
use std::fs;

fn main() -> Result<(), String> {
  let content = fs::read_to_string("calcit/compact.cirru").expect("expected a Cirru snapshot");
  let data = parse_cirru_edn(content.clone())?;
  // println!("reading: {}", content.clone());

  let s = snapshot::load_snapshot_data(data)?;

  println!("{:?}", s);

  println!("code: {:?}", program::extract_program_data(s)?);

  println!("{:?}", program::lookup_evaled_def("a", "b"));
  // TODO simulate program state
  program::write_evaled_def("a", "b", CalcitBool(true))?;
  println!("{:?}", program::lookup_evaled_def("a", "b"));

  Ok(())
}
