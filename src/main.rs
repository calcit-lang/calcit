#[macro_use]
extern crate lazy_static;

mod builtins;
mod data;
mod primes;
mod program;
mod runner;
mod snapshot;

use cirru_edn::parse_cirru_edn;
use im;
use primes::CalcitData;
use primes::CalcitData::*;
use std::fs;

fn main() -> Result<(), String> {
  let content = fs::read_to_string("calcit/compact.cirru").expect("expected a Cirru snapshot");
  let data = parse_cirru_edn(content.clone())?;
  // println!("reading: {}", content.clone());

  let bytes = include_bytes!("./cirru/calcit-core.cirru");
  print!("file: {}", String::from_utf8_lossy(bytes));

  let s = snapshot::load_snapshot_data(data)?.clone();

  // println!("{:?}", s);

  // println!("code: {:?}", );
  let program_code = program::extract_program_data(s.clone())?;

  // println!("{:?}", program::lookup_evaled_def("a", "b"));
  // TODO simulate program state
  // program::write_evaled_def("a", "b", CalcitBool(true))?;
  // println!("{:?}", program::lookup_evaled_def("a", "b"));

  let (init_ns, init_def) = extract_ns_def(s.configs.init_fn)?;
  match program::lookup_ns_def(&init_ns, &init_def, &program_code) {
    None => Err(String::from("Invalid entry")),
    Some(expr) => {
      // TODO faking test
      return Ok(());

      let entry = runner::evaluate_expr(expr, im::HashMap::new(), &init_ns, &program_code)?;
      match entry {
        CalcitFn(_, _, f) => {
          let result = f(vec![])?;
          println!("program result: {}", result);
          Ok(())
        }
        _ => Err(String::from("expected function entry")),
      }
    }
  }
}

fn extract_ns_def(s: String) -> Result<(String, String), String> {
  let pieces: Vec<&str> = (&s).split('/').collect();
  if pieces.len() == 2 {
    Ok((pieces[0].to_string(), pieces[1].to_string()))
  } else {
    Err(String::from("todo"))
  }
}
