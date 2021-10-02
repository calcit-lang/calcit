use std::collections::HashMap;
use std::fs;
use std::path::Path;

use cirru_edn::{format, Edn};

use crate::primes::{lookup_order_kwd_str, Calcit, CalcitItems, ImportRule, SymbolResolved::*};
use crate::program;

#[derive(Debug)]
struct IrDataImport {
  ns: String,
  kind: String,
  def: Option<String>,
}

impl IrDataImport {
  fn to_edn(&self) -> Edn {
    let mut xs: HashMap<Edn, Edn> = HashMap::new();
    xs.insert(edn_kwd("ns"), Edn::Str(self.ns.to_owned()));
    xs.insert(edn_kwd("kind"), Edn::Str(self.kind.to_owned()));
    match &self.def {
      Some(def) => xs.insert(edn_kwd("def"), Edn::Str(def.to_owned())),
      None => xs.insert(edn_kwd("def"), Edn::Nil),
    };
    Edn::Map(xs)
  }
}

#[derive(Debug)]
struct IrDataFile {
  import: HashMap<String, IrDataImport>,
  defs: HashMap<String, Edn>,
}

impl IrDataFile {
  fn to_edn(&self) -> Edn {
    let mut xs: HashMap<Edn, Edn> = HashMap::new();
    let mut import_data: HashMap<Edn, Edn> = HashMap::new();
    let mut defs_data: HashMap<Edn, Edn> = HashMap::new();

    for (k, v) in &self.import {
      import_data.insert(Edn::Str(k.to_owned()), v.to_edn());
    }

    for (k, v) in &self.defs {
      defs_data.insert(Edn::Str(k.to_owned()), v.to_owned());
    }

    xs.insert(edn_kwd("import"), Edn::Map(import_data));
    xs.insert(edn_kwd("defs"), Edn::Map(defs_data));
    Edn::Map(xs)
  }
}

#[derive(Debug)]
struct IrDataConfig {
  init_fn: String,
  reload_fn: String,
}

impl IrDataConfig {
  fn to_edn(&self) -> Edn {
    let mut xs: HashMap<Edn, Edn> = HashMap::new();
    xs.insert(edn_kwd("init-fn"), Edn::Str(self.init_fn.to_owned()));
    xs.insert(edn_kwd("reload-fn"), Edn::Str(self.reload_fn.to_owned()));
    Edn::Map(xs)
  }
}

#[derive(Debug)]
pub struct IrData {
  configs: IrDataConfig,
  files: HashMap<String, IrDataFile>,
}

impl IrData {
  fn to_edn(&self) -> Edn {
    let mut xs: HashMap<Edn, Edn> = HashMap::new();
    xs.insert(edn_kwd("configs"), self.configs.to_edn());
    let mut files: HashMap<Edn, Edn> = HashMap::new();
    for (k, v) in &self.files {
      files.insert(Edn::Str(k.to_owned()), v.to_edn());
    }
    xs.insert(edn_kwd("files"), Edn::Map(files));
    Edn::Map(xs)
  }
}

pub fn emit_ir(init_fn: &str, reload_fn: &str, emit_path: &str) -> Result<(), String> {
  let program_data = program::clone_evaled_program();

  let mut files: HashMap<String, IrDataFile> = HashMap::new();

  for (ns, file_info) in program_data {
    // TODO current implementation does not contain imports in evaled data
    let imports: HashMap<String, IrDataImport> = HashMap::new();

    let mut defs: HashMap<String, Edn> = HashMap::new();
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

  let content = match format(&data.to_edn(), true) {
    Ok(v) => v,
    Err(e) => return Err(format!("failed {}", e)),
  };

  let code_emit_path = Path::new(emit_path);
  if !code_emit_path.exists() {
    let _ = fs::create_dir(code_emit_path);
  }

  let js_file_path = code_emit_path.join("program-ir.cirru"); // TODO mjs_mode
  let _ = fs::write(&js_file_path, content);
  println!("wrote to: {}", js_file_path.to_str().unwrap());

  Ok(())
}

fn edn_kwd(s: &str) -> Edn {
  Edn::Keyword(s.to_string())
}

fn dump_code(code: &Calcit) -> Edn {
  match code {
    Calcit::Number(n) => Edn::Number(*n),
    Calcit::Nil => Edn::Nil,
    Calcit::Str(s) => Edn::Str(s.to_owned()),
    Calcit::Bool(b) => Edn::Bool(b.to_owned()),
    Calcit::Keyword(s) => edn_kwd(&lookup_order_kwd_str(s)),
    Calcit::Symbol(s, ns, at_def, resolved) => {
      let resolved = match resolved {
        Some(ResolvedDef(r_def, r_ns, import_rule)) => {
          let mut xs: HashMap<Edn, Edn> = HashMap::new();
          xs.insert(edn_kwd("kind"), Edn::Str(String::from("def")));
          xs.insert(edn_kwd("ns"), Edn::Str(r_ns.to_owned()));
          xs.insert(edn_kwd("at_def"), Edn::Str(at_def.to_owned()));
          xs.insert(edn_kwd("def"), Edn::Str(r_def.to_owned()));
          xs.insert(
            edn_kwd("rule"),
            match import_rule {
              Some(ImportRule::NsAs(_n)) => Edn::Str(String::from("ns")),
              Some(ImportRule::NsDefault(_n)) => Edn::Str(String::from("default")),
              Some(ImportRule::NsReferDef(_ns, _def)) => Edn::Str(String::from("def")),
              None => Edn::Nil,
            },
          );

          Edn::Map(xs)
        }
        Some(ResolvedLocal) => {
          let mut xs: HashMap<Edn, Edn> = HashMap::new();
          xs.insert(edn_kwd("kind"), edn_kwd("local"));
          Edn::Map(xs)
        }
        Some(ResolvedRaw) => {
          let mut xs: HashMap<Edn, Edn> = HashMap::new();
          xs.insert(edn_kwd("kind"), edn_kwd("raw"));
          Edn::Map(xs)
        }
        None => {
          let mut xs: HashMap<Edn, Edn> = HashMap::new();
          xs.insert(edn_kwd("kind"), Edn::Nil);
          Edn::Map(xs)
        }
      };

      let mut xs: HashMap<Edn, Edn> = HashMap::new();
      xs.insert(edn_kwd("kind"), edn_kwd("symbol"));
      xs.insert(edn_kwd("val"), Edn::Str(s.to_owned()));
      xs.insert(edn_kwd("ns"), Edn::Str(ns.to_owned()));
      xs.insert(edn_kwd("resolved"), resolved);
      Edn::Map(xs)
    }

    Calcit::Fn(name, ns, _id, _scope, args, body) => {
      let mut xs: HashMap<Edn, Edn> = HashMap::new();
      xs.insert(edn_kwd("kind"), Edn::Str(String::from("fn")));
      xs.insert(edn_kwd("name"), Edn::Str(name.to_string()));
      xs.insert(edn_kwd("ns"), Edn::Str(ns.to_string()));
      xs.insert(edn_kwd("args"), dump_items_code(args)); // TODO
      xs.insert(edn_kwd("code"), dump_items_code(body));
      Edn::Map(xs)
    }
    Calcit::Macro(name, ns, _id, args, body) => {
      let mut xs: HashMap<Edn, Edn> = HashMap::new();
      xs.insert(edn_kwd("kind"), Edn::Str(String::from("macro")));
      xs.insert(edn_kwd("name"), Edn::Str(name.to_string()));
      xs.insert(edn_kwd("ns"), Edn::Str(ns.to_string()));
      xs.insert(edn_kwd("args"), dump_items_code(args)); // TODO
      xs.insert(edn_kwd("code"), dump_items_code(body));
      Edn::Map(xs)
    }
    Calcit::Proc(name) => {
      let mut xs: HashMap<Edn, Edn> = HashMap::new();
      xs.insert(edn_kwd("kind"), Edn::Str(String::from("proc")));
      xs.insert(edn_kwd("name"), Edn::Str(name.to_string()));
      xs.insert(edn_kwd("builtin"), Edn::Bool(true));
      Edn::Map(xs)
    }
    Calcit::Syntax(name, _ns) => {
      let mut xs: HashMap<Edn, Edn> = HashMap::new();
      xs.insert(edn_kwd("kind"), Edn::Str(String::from("syntax")));
      xs.insert(edn_kwd("name"), Edn::Str(name.to_string()));
      Edn::Map(xs)
    }
    Calcit::Thunk(code, _) => dump_code(code),
    Calcit::List(xs) => {
      let mut ys: Vec<Edn> = vec![];
      for x in xs {
        ys.push(dump_code(x));
      }
      Edn::List(ys)
    }
    a => Edn::Str(format!("TODO {}", a)),
  }
}

fn dump_items_code(xs: &CalcitItems) -> Edn {
  let mut ys: Vec<Edn> = vec![];
  for x in xs {
    ys.push(dump_code(x));
  }
  Edn::List(ys)
}
