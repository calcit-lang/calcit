use std::collections::HashMap;
use std::fs;
use std::path::Path;

use cirru_edn::{format, Edn};

use crate::primes::{Calcit, CalcitItems, ImportRule, SymbolResolved::*};
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
    xs.insert(Edn::kwd("ns"), Edn::str(self.ns.to_owned()));
    xs.insert(Edn::kwd("kind"), Edn::str(self.kind.to_owned()));
    match &self.def {
      Some(def) => xs.insert(Edn::kwd("def"), Edn::str(def)),
      None => xs.insert(Edn::kwd("def"), Edn::Nil),
    };
    Edn::Map(xs)
  }
}

#[derive(Debug)]
struct IrDataFile {
  import: HashMap<Box<str>, IrDataImport>,
  defs: HashMap<Box<str>, Edn>,
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

    xs.insert(Edn::kwd("import"), Edn::Map(import_data));
    xs.insert(Edn::kwd("defs"), Edn::Map(defs_data));
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
    xs.insert(Edn::kwd("init-fn"), Edn::str(self.init_fn.to_owned()));
    xs.insert(Edn::kwd("reload-fn"), Edn::str(self.reload_fn.to_owned()));
    Edn::Map(xs)
  }
}

#[derive(Debug)]
pub struct IrData {
  configs: IrDataConfig,
  files: HashMap<Box<str>, IrDataFile>,
}

impl IrData {
  fn to_edn(&self) -> Edn {
    let mut xs: HashMap<Edn, Edn> = HashMap::new();
    xs.insert(Edn::kwd("configs"), self.configs.to_edn());
    let mut files: HashMap<Edn, Edn> = HashMap::new();
    for (k, v) in &self.files {
      files.insert(Edn::Str(k.to_owned()), v.to_edn());
    }
    xs.insert(Edn::kwd("files"), Edn::Map(files));
    Edn::Map(xs)
  }
}

pub fn emit_ir(init_fn: &str, reload_fn: &str, emit_path: &str) -> Result<(), String> {
  let program_data = program::clone_evaled_program();

  let mut files: HashMap<Box<str>, IrDataFile> = HashMap::new();

  for (ns, file_info) in program_data {
    // TODO current implementation does not contain imports in evaled data
    let imports: HashMap<Box<str>, IrDataImport> = HashMap::new();

    let mut defs: HashMap<Box<str>, Edn> = HashMap::new();
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

fn dump_code(code: &Calcit) -> Edn {
  match code {
    Calcit::Number(n) => Edn::Number(*n),
    Calcit::Nil => Edn::Nil,
    Calcit::Str(s) => Edn::Str(s.to_owned()),
    Calcit::Bool(b) => Edn::Bool(b.to_owned()),
    Calcit::Keyword(s) => Edn::Keyword(s.to_owned()),
    Calcit::Symbol(s, ns, at_def, resolved) => {
      let resolved = match resolved {
        Some(resolved) => match &**resolved {
          ResolvedDef(r_def, r_ns, import_rule) => {
            let mut xs: HashMap<Edn, Edn> = HashMap::new();
            xs.insert(Edn::kwd("kind"), Edn::kwd("def"));
            xs.insert(Edn::kwd("ns"), Edn::Str(r_ns.to_owned()));
            xs.insert(Edn::kwd("at_def"), Edn::Str(at_def.to_owned()));
            xs.insert(Edn::kwd("def"), Edn::Str(r_def.to_owned()));
            xs.insert(
              Edn::kwd("rule"),
              match import_rule {
                Some(ImportRule::NsAs(_n)) => Edn::kwd("ns"),
                Some(ImportRule::NsDefault(_n)) => Edn::kwd("default"),
                Some(ImportRule::NsReferDef(_ns, _def)) => Edn::kwd("def"),
                None => Edn::Nil,
              },
            );

            Edn::Map(xs)
          }
          ResolvedLocal => {
            let mut xs: HashMap<Edn, Edn> = HashMap::new();
            xs.insert(Edn::kwd("kind"), Edn::kwd("local"));
            Edn::Map(xs)
          }
          ResolvedRaw => {
            let mut xs: HashMap<Edn, Edn> = HashMap::new();
            xs.insert(Edn::kwd("kind"), Edn::kwd("raw"));
            Edn::Map(xs)
          }
        },
        None => {
          let mut xs: HashMap<Edn, Edn> = HashMap::new();
          xs.insert(Edn::kwd("kind"), Edn::Nil);
          Edn::Map(xs)
        }
      };

      let mut xs: HashMap<Edn, Edn> = HashMap::new();
      xs.insert(Edn::kwd("kind"), Edn::kwd("symbol"));
      xs.insert(Edn::kwd("val"), Edn::Str(s.to_owned()));
      xs.insert(Edn::kwd("ns"), Edn::Str(ns.to_owned()));
      xs.insert(Edn::kwd("resolved"), resolved);
      Edn::Map(xs)
    }

    Calcit::Fn(name, ns, _id, _scope, args, body) => {
      let mut xs: HashMap<Edn, Edn> = HashMap::new();
      xs.insert(Edn::kwd("kind"), Edn::kwd("fn"));
      xs.insert(Edn::kwd("name"), Edn::Str(name.to_owned()));
      xs.insert(Edn::kwd("ns"), Edn::Str(ns.to_owned()));
      xs.insert(Edn::kwd("args"), dump_items_code(args)); // TODO
      xs.insert(Edn::kwd("code"), dump_items_code(body));
      Edn::Map(xs)
    }
    Calcit::Macro(name, ns, _id, args, body) => {
      let mut xs: HashMap<Edn, Edn> = HashMap::new();
      xs.insert(Edn::kwd("kind"), Edn::kwd("macro"));
      xs.insert(Edn::kwd("name"), Edn::Str(name.to_owned()));
      xs.insert(Edn::kwd("ns"), Edn::Str(ns.to_owned()));
      xs.insert(Edn::kwd("args"), dump_items_code(args)); // TODO
      xs.insert(Edn::kwd("code"), dump_items_code(body));
      Edn::Map(xs)
    }
    Calcit::Proc(name) => {
      let mut xs: HashMap<Edn, Edn> = HashMap::new();
      xs.insert(Edn::kwd("kind"), Edn::kwd("proc"));
      xs.insert(Edn::kwd("name"), Edn::Str(name.to_owned()));
      xs.insert(Edn::kwd("builtin"), Edn::Bool(true));
      Edn::Map(xs)
    }
    Calcit::Syntax(name, _ns) => {
      let mut xs: HashMap<Edn, Edn> = HashMap::new();
      xs.insert(Edn::kwd("kind"), Edn::kwd("syntax"));
      xs.insert(Edn::kwd("name"), Edn::str(name.to_string()));
      Edn::Map(xs)
    }
    Calcit::Thunk(code, _) => dump_code(code),
    Calcit::List(xs) => {
      let mut ys: Vec<Edn> = Vec::with_capacity(xs.len());
      for x in xs {
        ys.push(dump_code(x));
      }
      Edn::List(ys)
    }
    a => Edn::str(format!("TODO {}", a)),
  }
}

fn dump_items_code(xs: &CalcitItems) -> Edn {
  let mut ys: Vec<Edn> = Vec::with_capacity(xs.len());
  for x in xs {
    ys.push(dump_code(x));
  }
  Edn::List(ys)
}
