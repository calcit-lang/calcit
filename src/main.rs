mod edn;
mod primes;
mod snapshot;

use cirru_edn::{parse_cirru_edn, CirruEdn};
use std::fs;

fn main() -> Result<(), String> {
  let content = fs::read_to_string("cirru/compact.cirru").expect("expected a Cirru snapshot");
  let data = parse_cirru_edn(content.clone())?;
  // println!("reading: {}", content.clone());

  let s = snapshot::load_snapshot_data(data);

  println!("{:?}", s);
  Ok(())
}
