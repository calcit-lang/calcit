//! Reader-facing definition data written into `program-ir.cirru` as `:sources`.
//!
//! The compiled `:files` section only holds lowered code. Tools that show a
//! program to people (calcit-viewer) also need the Snapshot source, docs,
//! declared schemas, the type the compiler settled on, dependency edges and
//! inferred types keyed by the same source paths `query type-at --path` uses.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::{Arc, LazyLock, RwLock};

use cirru_edn::{Edn, EdnListView};
use cirru_parser::Cirru;
use md5::{Digest, Md5};

use crate::calcit::{
  Calcit, CalcitFnTypeAnnotation, CalcitImport, CalcitLocal, CalcitSyntax, CalcitTypeAnnotation, LocatedWarning, SchemaKind,
};
use crate::call_stack::CallStackList;
use crate::program::{self, CompiledDefKind};
use crate::runner::preprocess;
use crate::snapshot::{self, ChangesDict, Snapshot};

/// Where a namespace comes from, which decides how much is emitted for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrSourceOrigin {
  /// Every definition, with per-path inferred types.
  Project,
  /// Only definitions held by the compiled program, without per-path types.
  Dep,
  /// Same as `Dep`, for the bundled `calcit.*` namespaces.
  Core,
}

impl IrSourceOrigin {
  fn tag(self) -> Edn {
    Edn::tag(match self {
      Self::Project => "project",
      Self::Dep => "dep",
      Self::Core => "core",
    })
  }
}

#[derive(Debug, Clone)]
struct IrSourceFile {
  origin: IrSourceOrigin,
  ns_code: Cirru,
  ns_doc: Arc<str>,
  defs: HashMap<Arc<str>, Cirru>,
}

#[derive(Debug, Clone, Default)]
struct IrSourceIndex {
  package: String,
  snapshot_path: String,
  files: HashMap<Arc<str>, IrSourceFile>,
}

static IR_SOURCES: LazyLock<RwLock<Option<IrSourceIndex>>> = LazyLock::new(|| RwLock::new(None));

/// Keep the Snapshot source for `:sources`. The program registry only holds
/// parsed code, where reader shorthand such as `'T` is already expanded and
/// no longer matches the source paths readers point at.
pub fn install_ir_sources(snapshot: &Snapshot, snapshot_path: &str, project_namespaces: &HashSet<String>) {
  let mut files = HashMap::with_capacity(snapshot.files.len());
  for (ns, file) in &snapshot.files {
    if ns.ends_with(".$meta") {
      continue;
    }
    let origin = if project_namespaces.contains(ns) {
      IrSourceOrigin::Project
    } else if ns == "calcit" || ns.starts_with("calcit.") {
      IrSourceOrigin::Core
    } else {
      IrSourceOrigin::Dep
    };
    files.insert(
      Arc::from(ns.as_str()),
      IrSourceFile {
        origin,
        ns_code: file.ns.code.to_owned(),
        ns_doc: Arc::from(file.ns.doc.as_str()),
        defs: file
          .defs
          .iter()
          .map(|(def, entry)| (Arc::from(def.as_str()), entry.code.to_owned()))
          .collect(),
      },
    );
  }
  *IR_SOURCES.write().expect("write IR sources") = Some(IrSourceIndex {
    package: snapshot.package.to_owned(),
    snapshot_path: snapshot_path.to_owned(),
    files,
  });
}

/// Mirror an incremental reload so `ir --watch` keeps writing current source.
pub fn apply_ir_source_changes(changes: &ChangesDict) {
  let mut guard = IR_SOURCES.write().expect("write IR sources");
  let Some(index) = guard.as_mut() else { return };
  for ns in &changes.removed {
    index.files.remove(ns);
  }
  for (ns, file) in &changes.added {
    index.files.insert(
      ns.to_owned(),
      IrSourceFile {
        origin: IrSourceOrigin::Project,
        ns_code: file.ns.code.to_owned(),
        ns_doc: Arc::from(file.ns.doc.as_str()),
        defs: file
          .defs
          .iter()
          .map(|(def, entry)| (Arc::from(def.as_str()), entry.code.to_owned()))
          .collect(),
      },
    );
  }
  for (ns, info) in &changes.changed {
    let Some(file) = index.files.get_mut(ns) else { continue };
    if let Some(code) = &info.ns {
      file.ns_code = code.to_owned();
    }
    for def in &info.removed_defs {
      file.defs.remove(def.as_str());
    }
    for (def, change) in info.added_defs.iter().chain(info.changed_defs.iter()) {
      file.defs.insert(Arc::from(def.as_str()), change.code().to_owned());
    }
  }
}

pub(crate) fn has_ir_sources() -> bool {
  IR_SOURCES.read().expect("read IR sources").is_some()
}

/// `:meta` describes the artifact so readers can check what produced it.
pub(crate) fn dump_meta(generated_at_ms: f64) -> Edn {
  let guard = IR_SOURCES.read().expect("read IR sources");
  let (package, snapshot_path) = guard
    .as_ref()
    .map(|index| (index.package.as_str(), index.snapshot_path.as_str()))
    .unwrap_or(("", ""));
  Edn::map_from_iter([
    (Edn::tag("schema-version"), Edn::Number(1.0)),
    (Edn::tag("calcit-version"), Edn::str(env!("CARGO_PKG_VERSION"))),
    (Edn::tag("package"), Edn::str(package)),
    (Edn::tag("snapshot"), Edn::str(snapshot_path)),
    (Edn::tag("generated-at"), Edn::Number(generated_at_ms)),
  ])
}

/// Build `:sources`. `compiled` lists the definitions the program held before
/// this call: library namespaces are limited to it, while project definitions
/// outside it are compiled here so readers also see their types and dependencies.
pub(crate) fn dump_sources(compiled: &HashMap<Arc<str>, HashSet<Arc<str>>>) -> Edn {
  let index = match IR_SOURCES.read().expect("read IR sources").as_ref() {
    Some(index) => index.clone(),
    None => return Edn::map_from_iter(Vec::<(Edn, Edn)>::new()),
  };
  let mut namespaces: Vec<&Arc<str>> = index.files.keys().collect();
  namespaces.sort();

  let mut entries: Vec<(Edn, Edn)> = Vec::new();
  for ns in namespaces {
    let file = &index.files[ns];
    let reached = compiled.get(ns);
    let mut def_names: Vec<&Arc<str>> = file
      .defs
      .keys()
      .filter(|def| file.origin == IrSourceOrigin::Project || reached.is_some_and(|defs| defs.contains(*def)))
      .collect();
    // An empty project namespace still has an `ns` form readers can show.
    if def_names.is_empty() && file.origin != IrSourceOrigin::Project {
      continue;
    }
    def_names.sort();
    let defs = def_names
      .into_iter()
      .map(|def| {
        let source = &file.defs[def];
        (Edn::str(def.as_ref()), dump_def(ns, def, source, file.origin))
      })
      .collect::<Vec<_>>();
    entries.push((
      Edn::str(ns.as_ref()),
      Edn::map_from_iter([
        (Edn::tag("origin"), file.origin.tag()),
        (Edn::tag("ns-doc"), Edn::str(file.ns_doc.as_ref())),
        (Edn::tag("ns-code"), Edn::Quote(file.ns_code.to_owned())),
        (Edn::tag("defs"), Edn::map_from_iter(defs)),
      ]),
    ));
  }
  Edn::map_from_iter(entries)
}

fn dump_def(ns: &str, def: &str, source: &Cirru, origin: IrSourceOrigin) -> Edn {
  let mut error: Option<String> = None;
  if origin == IrSourceOrigin::Project {
    let warnings = RefCell::new(vec![]);
    if let Err(failure) = preprocess::compile_source_def_for_snapshot(ns, def, &warnings, &CallStackList::default()) {
      error = Some(failure.msg.to_string());
    }
  }
  let compiled = program::lookup_compiled_def(ns, def);

  let declared = program::lookup_def_schema(ns, def);
  let schema = if snapshot::schema_annotation_is_missing(&declared) {
    Edn::Nil
  } else {
    Edn::str(type_display(&declared))
  };
  let effective = compiled
    .as_ref()
    .and_then(|_| program::lookup_codegen_type_hint(ns, def))
    .map(|annotation| Edn::str(type_display(&annotation)))
    .unwrap_or(Edn::Nil);
  let doc = program::lookup_def_doc(ns, def).unwrap_or_default();

  let deps = compiled
    .as_ref()
    .map(|compiled| collect_deps(&compiled.preprocessed_code, ns, def))
    .unwrap_or_default();

  let types = if origin == IrSourceOrigin::Project && error.is_none() {
    collect_path_types(ns, def, source, compiled.as_ref().map(|compiled| &compiled.preprocessed_code))
  } else {
    BTreeMap::new()
  };

  let schema_text = match &schema {
    Edn::Str(text) => text.to_string(),
    _ => String::new(),
  };
  let mut entries = vec![
    (Edn::tag("hash"), Edn::str(source_hash(source, &schema_text, &doc))),
    (Edn::tag("head"), source_head(source)),
    (
      Edn::tag("kind"),
      // Core syntax such as `defn` or `if` is registered as a runtime stub; name it for readers.
      if ns == crate::calcit::CORE_NS && CalcitSyntax::is_valid(def) {
        Edn::tag("syntax")
      } else {
        compiled
          .as_ref()
          .map(|compiled| compiled_kind_tag(&compiled.kind))
          .unwrap_or(Edn::Nil)
      },
    ),
    (Edn::tag("doc"), Edn::str(doc)),
    (Edn::tag("source"), Edn::Quote(source.to_owned())),
    (Edn::tag("schema"), schema),
    (Edn::tag("type"), effective),
    (Edn::tag("deps"), Edn::List(EdnListView(deps.into_iter().map(Edn::str).collect()))),
    (
      Edn::tag("types"),
      Edn::List(EdnListView(
        types
          .into_iter()
          .map(|(path, annotation)| Edn::List(EdnListView(vec![Edn::str(path), Edn::str(annotation)])))
          .collect(),
      )),
    ),
  ];
  if let Some(message) = error {
    entries.push((Edn::tag("error"), Edn::str(message)));
  }
  Edn::map_from_iter(entries)
}

fn compiled_kind_tag(kind: &CompiledDefKind) -> Edn {
  Edn::tag(match kind {
    CompiledDefKind::Proc => "proc",
    CompiledDefKind::Fn => "fn",
    CompiledDefKind::Macro => "macro",
    CompiledDefKind::Syntax => "syntax",
    CompiledDefKind::LazyValue => "lazy-value",
    CompiledDefKind::Value => "value",
  })
}

/// The leading leaf of a definition, such as `defn`, `defcomp` or `defstruct`.
fn source_head(source: &Cirru) -> Edn {
  match source {
    Cirru::List(items) => match items.first() {
      Some(Cirru::Leaf(head)) => Edn::str(head.as_ref()),
      _ => Edn::Nil,
    },
    Cirru::Leaf(_) => Edn::Nil,
  }
}

/// Stable per-definition digest so readers can refresh only what changed.
fn source_hash(source: &Cirru, schema: &str, doc: &str) -> String {
  let mut hasher = Md5::new();
  hasher.update(format!("{source:?}").as_bytes());
  hasher.update([0]);
  hasher.update(schema.as_bytes());
  hasher.update([0]);
  hasher.update(doc.as_bytes());
  format!("md5:{}", hex::encode(hasher.finalize()))
}

/// Compact type spelling (`Fn String (-> Number)`) when one exists, otherwise
/// the canonical EDN, matching what `query` commands print.
pub(crate) fn type_display(annotation: &CalcitTypeAnnotation) -> String {
  let edn = annotation.to_type_edn();
  match snapshot::compact_type_display(&edn) {
    Some(Cirru::Leaf(name)) => name.to_string(),
    Some(compact) => compact
      .format_one_liner()
      .map(|text| text.trim().to_owned())
      .unwrap_or_else(|_| annotation.to_brief_string()),
    None => cirru_edn::format(&edn, true)
      .map(|text| {
        let text = text.trim();
        text.strip_prefix("do ").unwrap_or(text).to_owned()
      })
      .unwrap_or_else(|_| annotation.to_brief_string()),
  }
}

/// Definitions referenced by compiled code, as sorted `ns/def` strings.
/// Uses the same collection as the compiler's dependency graph, so nominal
/// types named by typed decode calls are edges too.
fn collect_deps(code: &Calcit, ns: &str, def: &str) -> Vec<String> {
  let mut found: BTreeSet<String> = program::collect_compiled_dep_names(code)
    .into_iter()
    .map(|(dep_ns, dep_def)| format!("{dep_ns}/{dep_def}"))
    .collect();
  found.remove(&format!("{ns}/{def}"));
  found.into_iter().collect()
}

/// Inferred types keyed by dotted source path. Call results come from the
/// compiler's source expression trace, and the head symbol of a traced call to a
/// builtin or another definition gets that callee's signature. Locals keep their
/// preprocessing type, `Dynamic` included, at the coordinate of the symbol they
/// were read from; parameters and `let` names are locals too.
fn collect_path_types(ns: &str, def: &str, source: &Cirru, preprocessed: Option<&Calcit>) -> BTreeMap<String, String> {
  let mut found: BTreeMap<String, String> = BTreeMap::new();
  let warnings: RefCell<Vec<LocatedWarning>> = RefCell::new(vec![]);
  if let Ok(evidence) = preprocess::trace_snapshot_source_expressions(source, ns, def, &warnings, &CallStackList::default()) {
    for item in evidence {
      if item.location.ns.as_ref() != ns || item.location.def.as_ref() != def {
        continue;
      }
      if let Some(annotation) = &item.inferred_type {
        record_type(&mut found, &item.location.coord, annotation);
      }
      let mut head_path = item.location.coord.to_vec();
      head_path.push(0);
      if let Some(head) = source_leaf_at(source, &head_path)
        && !names_macro(ns, head)
        && let Some(annotation) = call_head_type(&item.processed)
      {
        record_type(&mut found, &head_path, &annotation);
      }
    }
  }
  fn walk(code: &Calcit, ns: &str, def: &str, found: &mut BTreeMap<String, String>) {
    match code {
      Calcit::Local(CalcitLocal {
        info,
        location: Some(coord),
        type_info,
        ..
      }) if info.at_ns.as_ref() == ns && info.at_def.as_ref() == def => {
        record_type(found, coord, type_info);
      }
      Calcit::List(xs) => xs.traverse(&mut |x| walk(x, ns, def, found)),
      Calcit::Fn { info, .. } => {
        for x in &info.body {
          walk(x, ns, def, found);
        }
      }
      _ => {}
    }
  }
  if let Some(code) = preprocessed {
    walk(code, ns, def, &mut found);
  }
  found
}

/// Keeps the first type seen at a path, except that a concrete type replaces `Dynamic`.
fn record_type(found: &mut BTreeMap<String, String>, coord: &[u16], annotation: &CalcitTypeAnnotation) {
  let text = type_display(annotation);
  let key = path_key(coord);
  match found.get(&key) {
    Some(existing) if existing != "Dynamic" || text == "Dynamic" => {}
    _ => {
      found.insert(key, text);
    }
  }
}

/// Signature of the callee at the head of a processed call: a builtin proc or an imported definition.
fn call_head_type(processed: &Calcit) -> Option<CalcitTypeAnnotation> {
  let Calcit::List(items) = processed else { return None };
  match items.first()? {
    Calcit::Proc(proc) => {
      let signature = proc.get_type_signature()?;
      Some(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: signature.arg_types.clone(),
        return_type: signature.return_type.clone(),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(Default::default()),
        runtime_arity: None,
      })))
    }
    Calcit::Import(CalcitImport { ns, def, .. }) => program::lookup_codegen_type_hint(ns, def).map(|annotation| (*annotation).clone()),
    _ => None,
  }
}

/// Whether a head symbol names a macro. Its call lowers to unrelated code, so
/// the lowered callee's signature would describe something the reader did not write.
fn names_macro(ns: &str, head: &str) -> bool {
  let target = match head.split_once('/') {
    Some((alias, def)) if !alias.is_empty() && !def.is_empty() => {
      program::lookup_ns_target_in_import(ns, alias).map(|target| (target.to_string(), def.to_owned()))
    }
    _ => {
      if program::has_def_code(ns, head) {
        Some((ns.to_owned(), head.to_owned()))
      } else if let Some(target) = program::lookup_def_target_in_import(ns, head) {
        Some((target.to_string(), head.to_owned()))
      } else {
        Some((crate::calcit::CORE_NS.to_owned(), head.to_owned()))
      }
    }
  };
  target.is_some_and(|(target_ns, target_def)| {
    matches!(
      program::lookup_def_code(&target_ns, &target_def),
      Some(Calcit::List(items)) if matches!(items.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "defmacro")
    )
  })
}

fn source_leaf_at<'a>(source: &'a Cirru, path: &[u16]) -> Option<&'a str> {
  let mut node = source;
  for idx in path {
    match node {
      Cirru::List(items) => node = items.get(*idx as usize)?,
      Cirru::Leaf(_) => return None,
    }
  }
  match node {
    Cirru::Leaf(text) => Some(text.as_ref()),
    Cirru::List(_) => None,
  }
}

fn path_key(coord: &[u16]) -> String {
  coord.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(".")
}

#[cfg(test)]
mod tests {
  use super::{path_key, source_hash};
  use cirru_parser::Cirru;

  #[test]
  fn path_keys_match_type_at_paths() {
    assert_eq!(path_key(&[3, 1, 0]), "3.1.0");
    assert_eq!(path_key(&[]), "");
  }

  #[test]
  fn source_hash_tracks_source_schema_and_doc() {
    let source = Cirru::List(vec![Cirru::leaf("defn"), Cirru::leaf("f"), Cirru::List(vec![])]);
    let base = source_hash(&source, "Fn (-> Unit)", "");
    assert_eq!(base, source_hash(&source, "Fn (-> Unit)", ""));
    assert_ne!(base, source_hash(&source, "Fn (-> Number)", ""));
    assert_ne!(base, source_hash(&source, "Fn (-> Unit)", "doc"));
  }
}
