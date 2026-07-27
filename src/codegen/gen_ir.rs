use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use cirru_edn::{Edn, EdnListView, format};

use crate::calcit::{
  Calcit, CalcitArgLabel, CalcitEnum, CalcitFnArgs, CalcitImpl, CalcitImport, CalcitLocal, CalcitRecord, CalcitStruct, CalcitTuple,
  CalcitTypeAnnotation, ImportInfo, MethodKind,
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

  let result = program::lookup_codegen_type_hint(ns, def)
    .map(|annotation| dump_type_annotation(annotation.as_ref()))
    .unwrap_or(Edn::Nil);

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
  let program_data = program::clone_compiled_program_snapshot()?;

  let mut files: HashMap<Arc<str>, IrDataFile> = HashMap::new();

  for (ns, file_info) in program_data.iter() {
    let mut defs: HashMap<Arc<str>, Edn> = HashMap::new();
    for (def, compiled) in &file_info.defs {
      defs.insert(def.to_owned(), dump_code(&compiled.codegen_form));
    }

    let file = IrDataFile { defs };
    files.insert(Arc::clone(ns), file);
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
    Calcit::Impl(impl_def) => dump_impl_code(impl_def),
    Calcit::Struct(struct_def) => dump_struct_code(struct_def),
    Calcit::Enum(enum_def) => dump_enum_code(enum_def),
    Calcit::Method(method, kind) => {
      let mut entries = vec![
        (Edn::tag("kind"), Edn::tag("method")),
        (Edn::tag("behavior"), Edn::Str((kind.to_string()).into())),
        (Edn::tag("method"), Edn::Str(method.to_owned())),
      ];
      if let MethodKind::Invoke(t) = kind
        && !matches!(**t, CalcitTypeAnnotation::Dynamic)
      {
        entries.push((Edn::tag("receiver-type"), dump_type_annotation(t.as_ref())));
      }
      Edn::map_from_iter(entries)
    }
    Calcit::Map(xs) => {
      // Map literals can appear as hint-fn schema data injected during preprocessing.
      let mut pairs = EdnListView::default();
      for (k, v) in xs.iter() {
        pairs.push(Edn::from(vec![dump_code(k), dump_code(v)]));
      }
      Edn::map_from_iter([(Edn::tag("kind"), Edn::tag("map")), (Edn::tag("pairs"), pairs.into())])
    }
    Calcit::AnyRef(_) => {
      // AnyRef is an opaque runtime handle; it cannot be embedded in IR code.
      Edn::map_from_iter([(Edn::tag("kind"), Edn::tag("any-ref"))])
    }
    Calcit::Set(xs) => {
      let mut items = EdnListView::default();
      for x in xs.iter() {
        items.push(dump_code(x));
      }
      Edn::map_from_iter([(Edn::tag("kind"), Edn::tag("set")), (Edn::tag("items"), items.into())])
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
  type_info.to_type_edn()
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

fn tuple_metadata_entries(tuple: &CalcitTuple) -> Vec<(Edn, Edn)> {
  let mut entries = vec![
    (Edn::tag("kind"), Edn::tag("tuple")),
    (Edn::tag("tag"), Edn::Str(tuple.tag.to_string().into())),
  ];
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

fn dump_impl_code(impl_def: &CalcitImpl) -> Edn {
  let mut entries = vec![
    (Edn::tag("kind"), Edn::tag("impl")),
    (Edn::tag("name"), Edn::Str(impl_def.name.ref_str().into())),
  ];
  if let Some(trait_def) = impl_def.origin() {
    entries.push((Edn::tag("trait"), Edn::Str(trait_def.name.ref_str().into())));
  }

  let mut fields = EdnListView::default();
  for (field, value) in impl_def.fields.iter().zip(impl_def.values.iter()) {
    fields.push(Edn::map_from_iter([
      (Edn::tag("field"), Edn::Str(field.ref_str().into())),
      (Edn::tag("value"), dump_code(value)),
    ]));
  }
  entries.push((Edn::tag("fields"), fields.into()));
  entries.push((Edn::tag("field-count"), Edn::Number(impl_def.fields.len() as f64)));
  Edn::map_from_iter(entries)
}

fn dump_struct_code(struct_def: &CalcitStruct) -> Edn {
  let mut entries = vec![
    (Edn::tag("kind"), Edn::tag("struct")),
    (Edn::tag("name"), Edn::Str(struct_def.name.ref_str().into())),
  ];
  {
    let mut impls_list = EdnListView::default();
    for imp in &struct_def.impls {
      impls_list.push(Edn::Str(imp.name().ref_str().into()));
    }
    if !impls_list.is_empty() {
      entries.push((Edn::tag("impls"), impls_list.into()));
    }
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
  {
    let mut impls_list = EdnListView::default();
    for imp in enum_def.impls() {
      impls_list.push(Edn::Str(imp.name().ref_str().into()));
    }
    if !impls_list.is_empty() {
      entries.push((Edn::tag("impls"), impls_list.into()));
    }
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
  let entries = vec![
    (Edn::tag("kind"), Edn::tag("record")),
    (Edn::tag("name"), Edn::Str(record.name().ref_str().into())),
  ];
  entries
}

#[cfg(test)]
mod tests {
  use super::{dump_code, dump_type_annotation};
  use crate::calcit::{Calcit, CalcitFnTypeAnnotation, CalcitImpl, CalcitProc, CalcitTypeAnnotation, SchemaKind};
  use cirru_edn::{Edn, EdnTag};
  use std::collections::HashSet;
  use std::sync::Arc;

  #[test]
  fn dumps_impl_values_for_ir() {
    let value = Calcit::Impl(CalcitImpl {
      name: EdnTag::new("DemoImpl"),
      origin: None,
      fields: Arc::new(vec![EdnTag::new("show")]),
      values: Arc::new(vec![Calcit::Proc(CalcitProc::NativeStr)]),
    });

    let dumped = dump_code(&value);
    let Edn::Map(entries) = dumped else {
      panic!("expected impl to dump as map");
    };

    assert_eq!(entries.get(&Edn::tag("kind")), Some(&Edn::tag("impl")));
    assert_eq!(entries.get(&Edn::tag("name")), Some(&Edn::str("DemoImpl")));
    assert_eq!(entries.get(&Edn::tag("field-count")), Some(&Edn::Number(1.0)));

    let Some(Edn::List(fields)) = entries.get(&Edn::tag("fields")) else {
      panic!("expected impl fields list");
    };
    assert_eq!(fields.len(), 1);

    let Some(Edn::Map(field_entry)) = fields.iter().next() else {
      panic!("expected impl field entry to be a map");
    };
    assert_eq!(field_entry.get(&Edn::tag("field")), Some(&Edn::str("show")));

    let Some(Edn::Map(proc_entry)) = field_entry.get(&Edn::tag("value")) else {
      panic!("expected impl field value to be a map");
    };
    assert_eq!(proc_entry.get(&Edn::tag("kind")), Some(&Edn::tag("proc")));
    assert_eq!(proc_entry.get(&Edn::tag("name")), Some(&Edn::str("&str")));
    assert_eq!(proc_entry.get(&Edn::tag("builtin")), Some(&Edn::Bool(true)));
  }

  #[test]
  fn dumps_type_annotations_as_canonical_type_edn() {
    let list_type = CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::String));
    assert_eq!(
      dump_type_annotation(&list_type),
      Edn::tuple(Edn::tag("list"), vec![Edn::tag("string")])
    );

    let map_type = CalcitTypeAnnotation::Map(Arc::new(CalcitTypeAnnotation::String), Arc::new(CalcitTypeAnnotation::Number));
    assert_eq!(
      dump_type_annotation(&map_type),
      Edn::tuple(Edn::tag("map"), vec![Edn::tag("string"), Edn::tag("number")])
    );

    let fn_type = CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      arg_types: vec![Arc::new(CalcitTypeAnnotation::String)],
      return_type: Arc::new(CalcitTypeAnnotation::Bool),
      fn_kind: SchemaKind::Fn,
      rest_type: None,
      features: Arc::new(HashSet::new()),
    }));
    assert_eq!(
      dump_type_annotation(&fn_type),
      Edn::tuple(
        Edn::tag("fn"),
        vec![Edn::map_from_iter([
          (Edn::tag("args"), Edn::List(cirru_edn::EdnListView(vec![Edn::tag("string")]))),
          (Edn::tag("return"), Edn::tag("bool")),
        ])]
      )
    );
  }
}
