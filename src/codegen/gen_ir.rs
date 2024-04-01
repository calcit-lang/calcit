use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use cirru_edn::{format, Edn, EdnListView};

use crate::calcit::{Calcit, CalcitArgLabel, CalcitFnArgs, CalcitImport, CalcitLocal, ImportInfo};
use crate::program;

#[derive(Debug)]
struct IrDataFile {
  defs: HashMap<Arc<str>, Edn>,
}

impl From<IrDataFile> for Edn {
  fn from(data: IrDataFile) -> Self {
    Edn::map_from_iter([(Edn::tag("defs"), data.defs.into())])
  }
}

#[derive(Debug, Clone)]
struct IrDataConfig {
  init_fn: String,
  reload_fn: String,
}

impl From<IrDataConfig> for Edn {
  fn from(x: IrDataConfig) -> Edn {
    Edn::map_from_iter([(Edn::tag("init-fn"), x.init_fn.into()), (Edn::tag("reload-fn"), x.reload_fn.into())])
  }
}

#[derive(Debug)]
pub struct IrData {
  configs: IrDataConfig,
  files: HashMap<Arc<str>, IrDataFile>,
}

impl From<IrData> for Edn {
  fn from(x: IrData) -> Edn {
    Edn::map_from_iter([(Edn::tag("configs"), x.configs.into()), (Edn::tag("files"), x.files.into())])
  }
}

pub fn emit_ir(init_fn: &str, reload_fn: &str, emit_path: &str) -> Result<(), String> {
  let program_data = program::clone_evaled_program();

  let mut files: HashMap<Arc<str>, IrDataFile> = HashMap::new();

  for (ns, file_info) in program_data.iter() {
    let mut defs: HashMap<Arc<str>, Edn> = HashMap::new();
    for (def, code) in file_info.iter() {
      defs.insert(def, dump_code(code));
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
    Err(e) => return Err(format!("failed {e}")),
  };

  let code_emit_path = Path::new(emit_path);
  if !code_emit_path.exists() {
    let _ = fs::create_dir(code_emit_path);
  }

  let js_file_path = code_emit_path.join("program-ir.cirru");
  let _ = fs::write(&js_file_path, content);
  println!("wrote to: {}", js_file_path.to_str().expect("extract path"));

  Ok(())
}

pub(crate) fn dump_code(code: &Calcit) -> Edn {
  match code {
    Calcit::Number(n) => Edn::Number(*n),
    Calcit::Nil => Edn::Nil,
    Calcit::Str(s) => Edn::Str((**s).into()),
    Calcit::Bool(b) => Edn::Bool(b.to_owned()),
    Calcit::Tag(s) => Edn::Tag(s.to_owned()),
    Calcit::Symbol { sym, info, location } => Edn::map_from_iter([
      (Edn::tag("kind"), Edn::tag("symbol")),
      (Edn::tag("val"), Edn::Str((**sym).into())),
      (Edn::tag("at-def"), Edn::Str((*info.at_def).into())),
      (Edn::tag("ns"), Edn::Str((*info.at_ns).into())),
      (
        Edn::tag("location"),
        match location {
          None => Edn::Nil,
          Some(xs) => Edn::from(xs.iter().map(|x| Edn::Number(*x as f64)).collect::<Vec<Edn>>()),
        },
      ),
    ]),
    Calcit::Local(CalcitLocal { sym, idx, info, .. }) => Edn::map_from_iter([
      (Edn::tag("kind"), Edn::tag("local")),
      (Edn::tag("val"), Edn::Str((**sym).into())),
      (Edn::tag("idx"), Edn::Number(*idx as f64)),
      (
        Edn::tag("info"),
        Edn::map_from_iter([
          (Edn::tag("at-def"), Edn::Str((*info.at_def).into())),
          (Edn::tag("ns"), Edn::Str((*info.at_ns).into())),
        ]),
      ),
    ]),

    Calcit::Import(CalcitImport { ns, def, info, .. }) => Edn::map_from_iter([
      (Edn::tag("kind"), Edn::tag("import")),
      (Edn::tag("ns"), Edn::Str((**ns).into())),
      (Edn::tag("def"), Edn::Str((**def).into())),
      (
        Edn::tag("info"),
        match &**info {
          ImportInfo::NsAs { alias, at_ns, at_def } => Edn::map_from_iter([
            (Edn::tag("kind"), Edn::tag("as")),
            (Edn::tag("alias"), Edn::Str((**alias).into())),
            (Edn::tag("at-ns"), Edn::Str((**at_ns).into())),
            (Edn::tag("at-def"), Edn::Str((**at_def).into())),
          ]),
          ImportInfo::JsDefault { alias, at_ns, at_def } => Edn::map_from_iter([
            (Edn::tag("kind"), Edn::tag("js-default")),
            (Edn::tag("alias"), Edn::Str((**alias).into())),
            (Edn::tag("at-ns"), Edn::Str((**at_ns).into())),
            (Edn::tag("at-def"), Edn::Str((**at_def).into())),
          ]),
          ImportInfo::NsReferDef { at_ns, at_def } => Edn::map_from_iter([
            (Edn::tag("kind"), Edn::tag("refer")),
            (Edn::tag("at-ns"), Edn::Str((**at_ns).into())),
            (Edn::tag("at-def"), Edn::Str((**at_def).into())),
          ]),
          ImportInfo::SameFile { at_def } => Edn::map_from_iter([
            (Edn::tag("kind"), Edn::tag("same-file")),
            (Edn::tag("at-def"), Edn::Str((**at_def).into())),
          ]),
          ImportInfo::Core { at_ns } => Edn::map_from_iter([
            (Edn::tag("kind"), Edn::tag("core")),
            (Edn::tag("at-ns"), Edn::Str((**at_ns).into())),
          ]),
        },
      ),
    ]),

    Calcit::Registered(alias) => Edn::map_from_iter([
      (Edn::tag("kind"), Edn::tag("registered")),
      (Edn::tag("alias"), Edn::Str((**alias).into())),
    ]),

    Calcit::Fn { info, .. } => {
      Edn::map_from_iter([
        (Edn::tag("kind"), Edn::tag("fn")),
        (Edn::tag("name"), Edn::Str((*info.name).into())),
        (Edn::tag("ns"), Edn::Str((*info.def_ns).into())),
        (Edn::tag("args"), dump_fn_args_code(&info.args)), // TODO
        (Edn::tag("code"), dump_items_code(&info.body)),
      ])
    }
    Calcit::Macro { info, .. } => {
      Edn::map_from_iter([
        (Edn::tag("kind"), Edn::tag("macro")),
        (Edn::tag("name"), Edn::Str((*info.name).into())),
        (Edn::tag("ns"), Edn::Str((*info.def_ns).into())),
        (Edn::tag("args"), dump_args_code(&info.args)), // TODO
        (Edn::tag("code"), dump_items_code(&info.body)),
      ])
    }
    Calcit::Proc(name) => Edn::map_from_iter([
      (Edn::tag("kind"), Edn::tag("proc")),
      (Edn::tag("name"), Edn::Str(name.to_string().into())),
      (Edn::tag("builtin"), Edn::Bool(true)),
    ]),
    Calcit::Syntax(name, _ns) => Edn::map_from_iter([
      (Edn::tag("kind"), Edn::tag("syntax")),
      (Edn::tag("name"), Edn::Str((name.to_string()).into())),
    ]),
    Calcit::Thunk(thunk) => dump_code(thunk.get_code()),
    Calcit::List(xs) => {
      let mut ys: Vec<Edn> = Vec::with_capacity(xs.len());
      xs.traverse(&mut |x| {
        ys.push(dump_code(x));
      });
      Edn::from(ys)
    }
    Calcit::Method(method, kind) => Edn::map_from_iter([
      (Edn::tag("kind"), Edn::tag("method")),
      (Edn::tag("behavior"), Edn::Str((kind.to_string()).into())),
      (Edn::tag("method"), Edn::Str(method.to_owned())),
    ]),
    Calcit::RawCode(_, code) => Edn::map_from_iter([
      (Edn::tag("kind"), Edn::tag("raw-code")),
      (Edn::tag("code"), Edn::Str(code.to_owned())),
    ]),
    Calcit::CirruQuote(code) => Edn::map_from_iter([(Edn::tag("kind"), Edn::tag("cirru-quote")), (Edn::tag("code"), code.into())]),
    a => unreachable!("invalid data for generating code: {:?}", a),
  }
}

fn dump_items_code(xs: &[Calcit]) -> Edn {
  let mut ys = EdnListView::default();
  for x in xs {
    ys.push(dump_code(x));
  }
  ys.into()
}

fn dump_fn_args_code(xs: &CalcitFnArgs) -> Edn {
  let mut ys = EdnListView::default();
  match xs {
    CalcitFnArgs::MarkedArgs(xs) => {
      for x in xs {
        ys.push(Edn::Str(x.to_string().into()));
      }
    }
    CalcitFnArgs::Args(xs) => {
      for x in xs {
        let sym = CalcitLocal::read_name(*x);
        ys.push(Edn::Str(sym.into()));
      }
    }
  }

  ys.into()
}

fn dump_args_code(xs: &[CalcitArgLabel]) -> Edn {
  let mut ys = EdnListView::default();
  for x in xs {
    ys.push(Edn::sym(&*x.to_string()));
  }
  ys.into()
}
