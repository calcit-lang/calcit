use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use cirru_edn::{Edn, EdnListView, format};

use crate::calcit::{
  Calcit, CalcitArgLabel, CalcitEnum, CalcitFnArgs, CalcitFnTypeAnnotation, CalcitImport, CalcitLocal, CalcitRecord, CalcitStruct,
  CalcitTuple, CalcitTypeAnnotation, ImportInfo, MethodKind,
};
use crate::program;

thread_local! {
  static TYPE_INFO_STACK: RefCell<Vec<(Arc<str>, Arc<str>)>> = const { RefCell::new(vec![]) };
}

/// Extract type information from a Calcit definition for IR output
/// Returns Edn representation of the type
fn extract_import_type_info(ns: &str, def: &str) -> Edn {
  let mut should_short_circuit = false;
  let mut pushed = false;
  TYPE_INFO_STACK.with(|stack| {
    let mut stack = stack.borrow_mut();
    if stack.iter().any(|(ns0, def0)| ns0.as_ref() == ns && def0.as_ref() == def) {
      should_short_circuit = true;
    } else {
      stack.push((Arc::from(ns), Arc::from(def)));
      pushed = true;
    }
  });

  if should_short_circuit {
    return Edn::Nil;
  }

  let result = match program::lookup_evaled_def(ns, def) {
    Some(value) => {
      let annotation = CalcitTypeAnnotation::from_calcit(&value);
      match annotation {
        CalcitTypeAnnotation::Dynamic => Edn::Nil,
        _ => dump_type_annotation(&annotation),
      }
    }
    None => Edn::Nil,
  };

  if pushed {
    TYPE_INFO_STACK.with(|stack| {
      let mut stack = stack.borrow_mut();
      let _ = stack.pop();
    });
  }

  result
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
      (Edn::tag("type-info"), dump_type_annotation_opt(type_info)),
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
      (Edn::tag("return-type"), dump_type_annotation_opt(&info.return_type)),
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
        entries.push((Edn::tag("return-type"), dump_type_annotation_opt(&type_sig.return_type)));
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
    Calcit::Struct(struct_def) => dump_struct_code(struct_def),
    Calcit::Enum(enum_def) => dump_enum_code(enum_def),
    Calcit::Method(method, kind) => {
      let mut entries = vec![
        (Edn::tag("kind"), Edn::tag("method")),
        (Edn::tag("behavior"), Edn::Str((kind.to_string()).into())),
        (Edn::tag("method"), Edn::Str(method.to_owned())),
      ];
      if let MethodKind::Invoke(t) = kind {
        if !matches!(**t, CalcitTypeAnnotation::Dynamic) {
          entries.push((Edn::tag("receiver-type"), dump_type_annotation(t.as_ref())));
        }
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

fn dump_type_annotation_opt(type_info: &Arc<CalcitTypeAnnotation>) -> Edn {
  if matches!(**type_info, CalcitTypeAnnotation::Dynamic) {
    Edn::Nil
  } else {
    dump_type_annotation(type_info)
  }
}

fn dump_type_list(xs: &[Arc<CalcitTypeAnnotation>]) -> Edn {
  let mut view = EdnListView::default();
  for x in xs {
    view.push(if matches!(**x, CalcitTypeAnnotation::Dynamic) {
      Edn::Nil
    } else {
      dump_type_annotation(x)
    });
  }
  view.into()
}

fn dump_type_annotation(type_info: &CalcitTypeAnnotation) -> Edn {
  match type_info {
    CalcitTypeAnnotation::Bool => type_tag_map("bool"),
    CalcitTypeAnnotation::Number => type_tag_map("number"),
    CalcitTypeAnnotation::String => type_tag_map("string"),
    CalcitTypeAnnotation::Symbol => type_tag_map("symbol"),
    CalcitTypeAnnotation::Tag => type_tag_map("tag"),
    CalcitTypeAnnotation::List(_) => type_tag_map("list"),
    CalcitTypeAnnotation::Map(_, _) => type_tag_map("map"),
    CalcitTypeAnnotation::DynFn => type_tag_map("fn"),
    CalcitTypeAnnotation::Ref(_) => type_tag_map("ref"),
    CalcitTypeAnnotation::Buffer => type_tag_map("buffer"),
    CalcitTypeAnnotation::CirruQuote => type_tag_map("cirru-quote"),
    CalcitTypeAnnotation::Record(record) => dump_record_type_summary(record.as_ref()),
    CalcitTypeAnnotation::Tuple(tuple) => dump_tuple_annotation(tuple.as_ref()),
    CalcitTypeAnnotation::DynTuple => type_tag_map("tuple"),
    CalcitTypeAnnotation::Fn(signature) => dump_function_type_annotation(signature.as_ref()),
    CalcitTypeAnnotation::Set(_) => type_tag_map("set"),
    CalcitTypeAnnotation::Variadic(inner) => {
      let mut entries = vec![(Edn::tag("type"), Edn::tag("variadic"))];
      entries.push((Edn::tag("inner"), dump_type_annotation(inner.as_ref())));
      Edn::map_from_iter(entries)
    }
    CalcitTypeAnnotation::Custom(value) => Edn::map_from_iter([
      (Edn::tag("type"), Edn::tag("custom")),
      (Edn::tag("value"), dump_code(value.as_ref())),
    ]),
    CalcitTypeAnnotation::Optional(inner) => {
      let mut entries = vec![(Edn::tag("type"), Edn::tag("optional"))];
      entries.push((Edn::tag("inner"), dump_type_annotation(inner.as_ref())));
      Edn::map_from_iter(entries)
    }
    CalcitTypeAnnotation::Dynamic => Edn::Nil,
    CalcitTypeAnnotation::TypeVar(name) => Edn::map_from_iter([
      (Edn::tag("type"), Edn::tag("type-var")),
      (Edn::tag("value"), Edn::Str(name.to_string().into())),
    ]),
    CalcitTypeAnnotation::Struct(struct_def) => Edn::map_from_iter([
      (Edn::tag("type"), Edn::tag("struct")),
      (Edn::tag("value"), dump_struct_code(struct_def.as_ref())),
    ]),
    CalcitTypeAnnotation::AppliedStruct { base, args } => {
      let mut entries = vec![(Edn::tag("type"), Edn::tag("struct"))];
      entries.push((Edn::tag("value"), dump_struct_code(base.as_ref())));
      let mut args_edn = EdnListView::default();
      for arg in args.iter() {
        args_edn.push(dump_type_annotation(arg.as_ref()));
      }
      entries.push((Edn::tag("args"), args_edn.into()));
      Edn::map_from_iter(entries)
    }
    CalcitTypeAnnotation::Enum(enum_def) => Edn::map_from_iter([
      (Edn::tag("type"), Edn::tag("enum")),
      (Edn::tag("value"), dump_enum_code(enum_def.as_ref())),
    ]),
  }
}

fn dump_function_type_annotation(signature: &CalcitFnTypeAnnotation) -> Edn {
  let mut entries = vec![(Edn::tag("type"), Edn::tag("fn"))];
  entries.push((Edn::tag("args"), dump_type_list(&signature.arg_types)));
  entries.push((Edn::tag("return"), dump_type_annotation_opt(&signature.return_type)));
  Edn::map_from_iter(entries)
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
  let mut entries = tuple_type_metadata_entries(tuple);
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
    entries.push((Edn::tag("class"), Edn::Str(class.name().ref_str().into())));
  }
  if let Some(sum_type) = &tuple.sum_type {
    entries.push((Edn::tag("enum"), Edn::Str(sum_type.name().ref_str().into())));
  }
  entries
}

fn tuple_type_metadata_entries(tuple: &CalcitTuple) -> Vec<(Edn, Edn)> {
  let mut entries = vec![
    (Edn::tag("type"), Edn::tag("tuple")),
    (Edn::tag("tag"), Edn::Str(tuple.tag.to_string().into())),
  ];
  if let Some(class) = &tuple.class {
    entries.push((Edn::tag("class"), Edn::Str(class.name().ref_str().into())));
  }
  if let Some(sum_type) = &tuple.sum_type {
    entries.push((Edn::tag("enum"), Edn::Str(sum_type.name().ref_str().into())));
  }
  entries
}

fn dump_record_code(record: &CalcitRecord) -> Edn {
  let mut entries = record_metadata(record);
  let mut fields = EdnListView::default();
  for (field, value) in record.struct_ref.fields.iter().zip(record.values.iter()) {
    fields.push(Edn::map_from_iter([
      (Edn::tag("field"), Edn::Str(field.ref_str().into())),
      (Edn::tag("value"), dump_code(value)),
    ]));
  }
  entries.push((Edn::tag("fields"), fields.into()));
  entries.push((Edn::tag("field-count"), Edn::Number(record.struct_ref.fields.len() as f64)));
  Edn::map_from_iter(entries)
}

fn dump_record_type_summary(record: &CalcitRecord) -> Edn {
  let mut entries = record_type_metadata(record);
  let mut fields = EdnListView::default();
  for (field, field_type) in record.struct_ref.fields.iter().zip(record.struct_ref.field_types.iter()) {
    fields.push(Edn::map_from_iter([
      (Edn::tag("field"), Edn::Str(field.ref_str().into())),
      (Edn::tag("type"), dump_type_annotation(field_type.as_ref())),
    ]));
  }
  entries.push((Edn::tag("fields"), fields.into()));
  entries.push((Edn::tag("field-count"), Edn::Number(record.struct_ref.fields.len() as f64)));
  Edn::map_from_iter(entries)
}

fn dump_struct_code(struct_def: &CalcitStruct) -> Edn {
  let mut entries = vec![
    (Edn::tag("kind"), Edn::tag("struct")),
    (Edn::tag("name"), Edn::Str(struct_def.name.ref_str().into())),
  ];
  if let Some(class) = &struct_def.class {
    entries.push((Edn::tag("class"), Edn::Str(class.name().ref_str().into())));
  }
  let mut fields = EdnListView::default();
  for (field, field_type) in struct_def.fields.iter().zip(struct_def.field_types.iter()) {
    fields.push(Edn::map_from_iter([
      (Edn::tag("field"), Edn::Str(field.ref_str().into())),
      (Edn::tag("type"), dump_type_annotation(field_type.as_ref())),
    ]));
  }
  entries.push((Edn::tag("fields"), fields.into()));
  entries.push((Edn::tag("field-count"), Edn::Number(struct_def.fields.len() as f64)));
  Edn::map_from_iter(entries)
}

fn dump_enum_code(enum_def: &CalcitEnum) -> Edn {
  let mut entries = vec![
    (Edn::tag("kind"), Edn::tag("enum")),
    (Edn::tag("name"), Edn::Str(enum_def.name().ref_str().into())),
  ];
  if let Some(class) = enum_def.class() {
    entries.push((Edn::tag("class"), Edn::Str(class.name().ref_str().into())));
  }

  let mut variants = EdnListView::default();
  for variant in enum_def.variants() {
    let mut payloads = EdnListView::default();
    for payload in variant.payload_types() {
      payloads.push(dump_type_annotation(payload.as_ref()));
    }
    variants.push(Edn::map_from_iter([
      (Edn::tag("tag"), Edn::Str(variant.tag.ref_str().into())),
      (Edn::tag("payloads"), payloads.into()),
    ]));
  }
  entries.push((Edn::tag("variants"), variants.into()));
  entries.push((Edn::tag("variant-count"), Edn::Number(enum_def.variants().len() as f64)));
  Edn::map_from_iter(entries)
}

fn record_metadata(record: &CalcitRecord) -> Vec<(Edn, Edn)> {
  let mut entries = vec![
    (Edn::tag("kind"), Edn::tag("record")),
    (Edn::tag("name"), Edn::Str(record.name().ref_str().into())),
  ];
  if let Some(class) = &record.class {
    entries.push((Edn::tag("class"), Edn::Str(class.name().ref_str().into())));
  }
  entries
}

fn record_type_metadata(record: &CalcitRecord) -> Vec<(Edn, Edn)> {
  let mut entries = vec![
    (Edn::tag("type"), Edn::tag("record")),
    (Edn::tag("name"), Edn::Str(record.name().ref_str().into())),
  ];
  if let Some(class) = &record.class {
    entries.push((Edn::tag("class"), Edn::Str(class.name().ref_str().into())));
  }
  entries
}

fn type_tag_map(type_name: &str) -> Edn {
  Edn::map_from_iter([(Edn::tag("type"), Edn::tag(type_name))])
}
