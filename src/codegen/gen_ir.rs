use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use cirru_edn::{format, Edn};

use crate::primes::{Calcit, CalcitItems, ImportRule, SymbolResolved::*};
use crate::program;

#[derive(Debug)]
struct IrDataFile {
  defs: HashMap<Arc<str>, Edn>,
}

impl From<IrDataFile> for Edn {
  fn from(data: IrDataFile) -> Self {
    Edn::map_from_iter([(Edn::kwd("defs"), data.defs.into())])
  }
}

#[derive(Debug, Clone)]
struct IrDataConfig {
  init_fn: String,
  reload_fn: String,
}

impl From<IrDataConfig> for Edn {
  fn from(x: IrDataConfig) -> Edn {
    Edn::map_from_iter([(Edn::kwd("init-fn"), x.init_fn.into()), (Edn::kwd("reload-fn"), x.reload_fn.into())])
  }
}

#[derive(Debug)]
pub struct IrData {
  configs: IrDataConfig,
  files: HashMap<Arc<str>, IrDataFile>,
}

impl From<IrData> for Edn {
  fn from(x: IrData) -> Edn {
    Edn::map_from_iter([(Edn::kwd("configs"), x.configs.into()), (Edn::kwd("files"), x.files.into())])
  }
}

pub fn emit_ir(init_fn: &str, reload_fn: &str, emit_path: &str) -> Result<(), String> {
  let program_data = program::clone_evaled_program();

  let mut files: HashMap<Arc<str>, IrDataFile> = HashMap::new();

  for (ns, file_info) in program_data {
    let mut defs: HashMap<Arc<str>, Edn> = HashMap::new();
    for (def, code) in file_info {
      defs.insert(def, dump_code(&code));
    }

    let file = IrDataFile { defs };
    files.insert(ns, file);
  }

  let data = IrData {
    configs: IrDataConfig {
      init_fn: init_fn.to_owned(),
      reload_fn: reload_fn.to_owned(),
    },
    files,
  };

  let content = match format(&data.into(), true) {
    Ok(v) => v,
    Err(e) => return Err(format!("failed {}", e)),
  };

  let code_emit_path = Path::new(emit_path);
  if !code_emit_path.exists() {
    let _ = fs::create_dir(code_emit_path);
  }

  let js_file_path = code_emit_path.join("program-ir.cirru");
  let _ = fs::write(&js_file_path, content);
  println!("wrote to: {}", js_file_path.to_str().unwrap());

  Ok(())
}

pub(crate) fn dump_code(code: &Calcit) -> Edn {
  match code {
    Calcit::Number(n) => Edn::Number(*n),
    Calcit::Nil => Edn::Nil,
    Calcit::Str(s) => Edn::Str((**s).into()),
    Calcit::Bool(b) => Edn::Bool(b.to_owned()),
    Calcit::Keyword(s) => Edn::Keyword(s.to_owned()),
    Calcit::Symbol {
      sym,
      ns,
      at_def,
      resolved,
      location,
    } => {
      let resolved = match resolved {
        Some(resolved) => match &**resolved {
          ResolvedDef {
            ns: r_ns,
            def: r_def,
            rule: import_rule,
          } => Edn::map_from_iter([
            (Edn::kwd("kind"), Edn::kwd("def")),
            (Edn::kwd("ns"), Edn::Str((**r_ns).into())),
            (Edn::kwd("at_def"), Edn::Str((**at_def).into())),
            (Edn::kwd("def"), Edn::Str((**r_def).into())),
            (
              Edn::kwd("rule"),
              match import_rule.to_owned().map(|x| (&*x).to_owned()) {
                Some(ImportRule::NsAs(_n)) => Edn::kwd("ns"),
                Some(ImportRule::NsDefault(_n)) => Edn::kwd("default"),
                Some(ImportRule::NsReferDef(_ns, _def)) => Edn::kwd("def"),
                None => Edn::Nil,
              },
            ),
            (
              Edn::kwd("location"),
              match location {
                Some(xs) => xs.to_owned().into(),
                None => Edn::Nil,
              },
            ),
          ]),
          ResolvedLocal => Edn::map_from_iter([("kind".into(), Edn::kwd("local"))]),
          ResolvedRaw => Edn::map_from_iter([("kind".into(), Edn::kwd("raw"))]),
        },
        None => Edn::map_from_iter([("kind".into(), Edn::Nil)]),
      };

      Edn::map_from_iter([
        (Edn::kwd("kind"), Edn::kwd("symbol")),
        (Edn::kwd("val"), Edn::Str((**sym).into())),
        (Edn::kwd("ns"), Edn::Str((**ns).into())),
        (Edn::kwd("resolved"), resolved),
      ])
    }

    Calcit::Fn {
      name, def_ns, args, body, ..
    } => {
      Edn::map_from_iter([
        (Edn::kwd("kind"), Edn::kwd("fn")),
        (Edn::kwd("name"), Edn::Str((**name).into())),
        (Edn::kwd("ns"), Edn::Str((**def_ns).into())),
        (Edn::kwd("args"), dump_args_code(args)), // TODO
        (Edn::kwd("code"), dump_items_code(body)),
      ])
    }
    Calcit::Macro {
      name, def_ns, args, body, ..
    } => {
      Edn::map_from_iter([
        (Edn::kwd("kind"), Edn::kwd("macro")),
        (Edn::kwd("name"), Edn::Str((**name).into())),
        (Edn::kwd("ns"), Edn::Str((**def_ns).into())),
        (Edn::kwd("args"), dump_args_code(args)), // TODO
        (Edn::kwd("code"), dump_items_code(body)),
      ])
    }
    Calcit::Proc(name) => Edn::map_from_iter([
      (Edn::kwd("kind"), Edn::kwd("proc")),
      (Edn::kwd("name"), Edn::Str((**name).into())),
      (Edn::kwd("builtin"), Edn::Bool(true)),
    ]),
    Calcit::Syntax(name, _ns) => Edn::map_from_iter([
      (Edn::kwd("kind"), Edn::kwd("syntax")),
      (Edn::kwd("name"), Edn::Str((name.to_string()).into())),
    ]),
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

fn dump_args_code(xs: &[Arc<str>]) -> Edn {
  let mut ys: Vec<Edn> = Vec::with_capacity(xs.len());
  for x in xs {
    ys.push(Edn::sym(&*x.to_owned()));
  }
  Edn::List(ys)
}
