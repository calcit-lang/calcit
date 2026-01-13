use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use cirru_edn::{Edn, EdnListView, format};

use crate::calcit::{
  Calcit, CalcitArgLabel, CalcitFnArgs, CalcitImport, CalcitLocal, CalcitRecord, CalcitTuple, ImportInfo, MethodKind,
};
use crate::program;

/// Extract type information from a Calcit definition for IR output
/// Returns Edn representation of the type
fn extract_import_type_info(ns: &str, def: &str) -> Edn {
  match program::lookup_evaled_def(ns, def) {
    Some(Calcit::Fn { info, .. }) => {
      // For functions: (:: :fn ([] :t1 :t2 ...) :ret)
      let mut arg_types = EdnListView::default();
      for opt_t in &info.arg_types {
        match opt_t {
          Some(t) => arg_types.push(dump_type_annotation(t)),
          None => arg_types.push(Edn::Nil),
        }
      }

      let return_type = match &info.return_type {
        Some(t) => dump_type_annotation(t),
        None => Edn::Nil,
      };

      Edn::tuple(Edn::tag("fn"), vec![arg_types.into(), return_type])
    }
    Some(Calcit::Proc(name)) => {
      // For proc (builtin functions), extract type signature
      if let Some(type_sig) = name.get_type_signature() {
        let mut arg_types = EdnListView::default();
        for opt_t in &type_sig.arg_types {
          match opt_t {
            Some(t) => arg_types.push(dump_type_annotation(t)),
            None => arg_types.push(Edn::Nil),
          }
        }

        let return_type = match &type_sig.return_type {
          Some(t) => dump_type_annotation(t),
          None => Edn::Nil,
        };

        Edn::tuple(Edn::tag("fn"), vec![arg_types.into(), return_type])
      } else {
        Edn::Nil
      }
    }
    // For other values, output their type
    Some(value) => {
      // Simple type tag based on value kind
      match value {
        Calcit::Nil => Edn::tag("nil"),
        Calcit::Bool(_) => Edn::tag("bool"),
        Calcit::Number(_) => Edn::tag("number"),
        Calcit::Str(_) => Edn::tag("string"),
        Calcit::Tag(_) => Edn::tag("keyword"),
        Calcit::List(_) => Edn::tag("list"),
        Calcit::Map(_) => Edn::tag("map"),
        Calcit::Set(_) => Edn::tag("set"),
        Calcit::Record { .. } => Edn::tag("record"),
        Calcit::Tuple { .. } => Edn::tag("tuple"),
        Calcit::Ref(_, _) => Edn::tag("ref"),
        _ => Edn::Nil,
      }
    }
    None => Edn::Nil,
  }
}

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
    Calcit::Local(CalcitLocal {
      sym, idx, info, type_info, ..
    }) => Edn::map_from_iter([
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
      (Edn::tag("type-info"), dump_optional_type_annotation(type_info)),
    ]),

    Calcit::Import(CalcitImport { ns, def, info, .. }) => Edn::map_from_iter([
      (Edn::tag("kind"), Edn::tag("import")),
      (Edn::tag("ns"), Edn::Str((**ns).into())),
      (Edn::tag("def"), Edn::Str((**def).into())),
      (Edn::tag("type-hint"), extract_import_type_info(ns, def)),
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

    Calcit::Fn { info, .. } => Edn::map_from_iter([
      (Edn::tag("kind"), Edn::tag("fn")),
      (Edn::tag("name"), Edn::Str((*info.name).into())),
      (Edn::tag("ns"), Edn::Str((*info.def_ns).into())),
      (Edn::tag("args"), dump_fn_args_code(&info.args)),
      (Edn::tag("arg-types"), dump_type_list(&info.arg_types)),
      (Edn::tag("return-type"), dump_optional_type_annotation(&info.return_type)),
      (Edn::tag("code"), dump_items_code(&info.body)),
    ]),
    Calcit::Macro { info, .. } => {
      Edn::map_from_iter([
        (Edn::tag("kind"), Edn::tag("macro")),
        (Edn::tag("name"), Edn::Str((*info.name).into())),
        (Edn::tag("ns"), Edn::Str((*info.def_ns).into())),
        (Edn::tag("args"), dump_args_code(&info.args)), // TODO
        (Edn::tag("code"), dump_items_code(&info.body)),
      ])
    }
    Calcit::Proc(name) => {
      let mut entries = vec![
        (Edn::tag("kind"), Edn::tag("proc")),
        (Edn::tag("name"), Edn::Str(name.to_string().into())),
        (Edn::tag("builtin"), Edn::Bool(true)),
      ];

      // Add type signature if available
      if let Some(type_sig) = name.get_type_signature() {
        entries.push((Edn::tag("arg-types"), dump_type_list(&type_sig.arg_types)));
        entries.push((Edn::tag("return-type"), dump_optional_type_annotation(&type_sig.return_type)));
      }

      Edn::map_from_iter(entries)
    }
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
    Calcit::Tuple(tuple) => dump_tuple_code(tuple),
    Calcit::Record(record) => dump_record_code(record),
    Calcit::Method(method, kind) => {
      let mut entries = vec![
        (Edn::tag("kind"), Edn::tag("method")),
        (Edn::tag("behavior"), Edn::Str((kind.to_string()).into())),
        (Edn::tag("method"), Edn::Str(method.to_owned())),
      ];
      if let MethodKind::Invoke(Some(t)) = kind {
        entries.push((Edn::tag("receiver-type"), dump_type_annotation(t)));
      }
      Edn::map_from_iter(entries)
    }
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

fn dump_optional_type_annotation(type_info: &Option<Arc<Calcit>>) -> Edn {
  match type_info {
    Some(t) => dump_type_annotation(t),
    None => Edn::Nil,
  }
}

fn dump_type_list(xs: &[Option<Arc<Calcit>>]) -> Edn {
  let mut view = EdnListView::default();
  for x in xs {
    view.push(match x {
      Some(t) => dump_type_annotation(t),
      None => Edn::Nil,
    });
  }
  view.into()
}

fn dump_type_annotation(type_info: &Calcit) -> Edn {
  match type_info {
    Calcit::Record(record) => dump_record_type_summary(record),
    Calcit::Tuple(tuple) => dump_tuple_annotation(tuple),
    other => dump_code(other),
  }
}

fn dump_tuple_code(tuple: &CalcitTuple) -> Edn {
  let mut entries = tuple_metadata_entries(tuple);
  let mut values = EdnListView::default();
  for value in &tuple.extra {
    values.push(dump_code(value));
  }
  entries.push((Edn::tag("values"), values.into()));
  entries.push((Edn::tag("payload-size"), Edn::Number(tuple.extra.len() as f64)));
  Edn::map_from_iter(entries)
}

fn dump_tuple_annotation(tuple: &CalcitTuple) -> Edn {
  let mut entries = tuple_metadata_entries(tuple);
  let mut payload = EdnListView::default();
  for hint in &tuple.extra {
    payload.push(dump_code(hint));
  }
  entries.push((Edn::tag("payload"), payload.into()));
  entries.push((Edn::tag("payload-size"), Edn::Number(tuple.extra.len() as f64)));
  Edn::map_from_iter(entries)
}

fn tuple_metadata_entries(tuple: &CalcitTuple) -> Vec<(Edn, Edn)> {
  let mut entries = vec![
    (Edn::tag("kind"), Edn::tag("tuple")),
    (Edn::tag("tag"), Edn::Str(tuple.tag.to_string().into())),
  ];
  if let Some(class) = &tuple.class {
    entries.push((Edn::tag("class"), Edn::Str(class.name.ref_str().into())));
  }
  if let Some(sum_type) = &tuple.sum_type {
    entries.push((Edn::tag("enum"), Edn::Str(sum_type.name().ref_str().into())));
  }
  entries
}

fn dump_record_code(record: &CalcitRecord) -> Edn {
  let mut entries = record_metadata(record);
  let mut fields = EdnListView::default();
  for (field, value) in record.fields.iter().zip(record.values.iter()) {
    fields.push(Edn::map_from_iter([
      (Edn::tag("field"), Edn::Str(field.ref_str().into())),
      (Edn::tag("value"), dump_code(value)),
    ]));
  }
  entries.push((Edn::tag("fields"), fields.into()));
  entries.push((Edn::tag("field-count"), Edn::Number(record.fields.len() as f64)));
  Edn::map_from_iter(entries)
}

fn dump_record_type_summary(record: &CalcitRecord) -> Edn {
  let mut entries = record_metadata(record);
  let mut names = EdnListView::default();
  for field in record.fields.iter() {
    names.push(Edn::Str(field.ref_str().into()));
  }
  entries.push((Edn::tag("fields"), names.into()));
  entries.push((Edn::tag("field-count"), Edn::Number(record.fields.len() as f64)));
  Edn::map_from_iter(entries)
}

fn record_metadata(record: &CalcitRecord) -> Vec<(Edn, Edn)> {
  let mut entries = vec![
    (Edn::tag("kind"), Edn::tag("record")),
    (Edn::tag("name"), Edn::Str(record.name.ref_str().into())),
  ];
  if let Some(class) = &record.class {
    entries.push((Edn::tag("class"), Edn::Str(class.name.ref_str().into())));
  }
  entries
}
