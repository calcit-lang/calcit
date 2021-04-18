#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate nanoid;

mod builtins;
mod data;
mod primes;
mod program;
mod runner;
mod snapshot;

use cirru_edn::parse_cirru_edn;
use primes::CalcitData::*;
use std::fs;

fn main() -> Result<(), String> {
  let content = fs::read_to_string("calcit/compact.cirru").expect("expected a Cirru snapshot");
  let data = parse_cirru_edn(content)?;
  // println!("reading: {}", content);

  let bytes = include_bytes!("./cirru/calcit-core.cirru");
  let core_content = String::from_utf8_lossy(bytes).to_string();
  let core_data = parse_cirru_edn(core_content)?;
  let core_snapshot = snapshot::load_snapshot_data(core_data)?;

  let mut snapshot = snapshot::load_snapshot_data(data)?;
  for (k, v) in core_snapshot.files {
    snapshot.files.insert(k, v.clone());
  }

  // println!("{:?}", s);

  // println!("code: {:?}", );
  let program_code = program::extract_program_data(&snapshot)?;

  // println!("{:?}", program::lookup_evaled_def("a", "b"));
  // TODO simulate program state
  // program::write_evaled_def("a", "b", CalcitBool(true))?;
  // println!("{:?}", program::lookup_evaled_def("a", "b"));

  let (init_ns, init_def) = extract_ns_def(snapshot.configs.init_fn)?;
  match program::lookup_ns_def(&init_ns, &init_def, &program_code) {
    None => Err(String::from("Invalid entry")),
    Some(expr) => {
      let entry = runner::evaluate_expr(&expr, &im::HashMap::new(), &init_ns, &program_code)?;
      match entry {
        CalcitFn(_, _, def_scope, args, body) => {
          let result = runner::run_fn(
            im::Vector::new(),
            &def_scope,
            args,
            body,
            &init_ns,
            &program_code,
          )?;
          println!("result: {}", result);
          Ok(())
        }
        _ => Err(format!("expected function entry, got: {}", entry)),
      }
    }
  }
}

fn extract_ns_def(s: String) -> Result<(String, String), String> {
  let pieces: Vec<&str> = (&s).split('/').collect();
  if pieces.len() == 2 {
    Ok((pieces[0].to_string(), pieces[1].to_string()))
  } else {
    Err(format!("invalid ns format: {}", s))
  }
}
