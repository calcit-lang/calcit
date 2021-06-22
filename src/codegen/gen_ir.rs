use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::primes::{Calcit, CalcitItems, ImportRule, SymbolResolved::*};
use crate::program;

#[derive(Serialize, Deserialize)]
struct IrDataImport {
  ns: String,
  kind: String,
  def: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct IrDataFile {
  import: HashMap<String, IrDataImport>,
  defs: HashMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct IrDataConfig {
  init_fn: String,
  reload_fn: String,
}

#[derive(Serialize, Deserialize)]
pub struct IrData {
  configs: IrDataConfig,
  files: HashMap<String, IrDataFile>,
}

pub fn emit_ir(init_fn: &str, reload_fn: &str, emit_path: &str) -> Result<(), String> {
  let program_data = program::clone_evaled_program();

  let mut files: HashMap<String, IrDataFile> = HashMap::new();

  for (ns, file_info) in program_data {
    // TODO current implementation does not contain imports in evaled data
    let imports: HashMap<String, IrDataImport> = HashMap::new();

    let mut defs: HashMap<String, serde_json::Value> = HashMap::new();
    for (def, code) in file_info {
      defs.insert(def, dump_code(&code));
    }

    let file = IrDataFile { import: imports, defs };
    files.insert(ns, file);
  }

  let data = IrData {
    configs: IrDataConfig {
      init_fn: init_fn.to_owned(),
      reload_fn: reload_fn.to_owned(),
    },
    files,
  };

  let content = match serde_json::to_string(&data) {
    Ok(v) => v,
    Err(e) => return Err(format!("failed {}", e)),
  };

  let code_emit_path = Path::new(emit_path);
  if !code_emit_path.exists() {
    let _ = fs::create_dir(code_emit_path);
  }

  let js_file_path = code_emit_path.join("program-ir.json"); // TODO mjs_mode
  let _ = fs::write(&js_file_path, content);
  println!("wrote to: {}", js_file_path.to_str().unwrap());

  Ok(())
}

fn dump_code(code: &Calcit) -> serde_json::Value {
  match code {
    Calcit::Number(n) => json!(n),
    Calcit::Nil => serde_json::Value::Null,
    Calcit::Str(s) => json!(s),
    Calcit::Bool(b) => json!(b),
    Calcit::Keyword(s) => json!({
      "kind": "keyword",
      "val": s,
    }),
    Calcit::Symbol(s, ns, resolved) => json!({
      "kind": "symbol",
      "val": s,
      "ns": ns,
      "resolved": match resolved {
        Some(ResolvedDef(r_def, r_ns, import_rule)) => json!({
          "kind": "def",
          "ns": r_ns,
          "def": r_def,
          "rule": match import_rule {
            Some(ImportRule::NsAs(_n)) => json!("ns"),
            Some(ImportRule::NsDefault(_n)) => json!("default"),
            Some(ImportRule::NsReferDef(_ns, _def)) => json!("def"),
            None => serde_json::Value::Null,
          }
        }),
        Some(ResolvedLocal) => json!({
          "kind": "local"
        }),
        Some(ResolvedRaw) => json!({
          "kind": "raw"
        }) ,
        None => json!({
          "kind": serde_json::Value::Null
        })
      }
    }),

    Calcit::Fn(name, ns, _id, _scope, args, body) => {
      json!({
        "kind": "fn",
        "name": name,
        "ns": ns,
        "args": dump_items_code(&args), // TODO
        "code": dump_items_code(&body),
      })
    }
    Calcit::Macro(name, ns, _id, args, body) => {
      json!({
        "kind": "fn",
        "name": name,
        "ns": ns,
        "args": dump_items_code(&args), // TODO
        "code": dump_items_code(&body),
      })
    }
    Calcit::Proc(name) => {
      json!({
        "kind": "fn",
        "name": name,
        "builtin": json!(true)
      })
    }
    Calcit::Syntax(name, _ns) => {
      json!({
        "kind": "syntax",
        "name": name,
      })
    }
    Calcit::Thunk(code, _) => dump_code(code),
    Calcit::List(xs) => {
      let mut ys: Vec<serde_json::Value> = vec![];
      for x in xs {
        ys.push(dump_code(x));
      }
      json!(ys)
    }
    a => json!(format!("TODO {}", a)),
  }
}

fn dump_items_code(xs: &CalcitItems) -> serde_json::Value {
  let mut ys: Vec<serde_json::Value> = vec![];
  for x in xs {
    ys.push(dump_code(x));
  }
  json!(ys)
}
