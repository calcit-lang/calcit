mod entry_book;

#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::RwLock;

use cirru_parser::Cirru;

use crate::calcit::data_shape::DataShapeGraph;
use crate::calcit::{
  self, Calcit, CalcitErr, CalcitScope, CalcitSyntax, CalcitThunk, CalcitThunkInfo, CalcitTypeAnnotation, DYNAMIC_TYPE,
};
use crate::call_stack::CallStackList;
use crate::data::{cirru::code_to_calcit, data_to_calcit};
use crate::runner;
use crate::snapshot;
use crate::snapshot::Snapshot;
use crate::util::string::extract_pkg_from_ns;

pub use entry_book::EntryBook;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCell {
  Cold,
  Resolving,
  Lazy { code: Arc<Calcit>, info: Arc<CalcitThunkInfo> },
  Ready(Calcit),
  Errored(Arc<str>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeResolveMode {
  Strict,
  Lenient,
}

#[derive(Debug, Clone)]
pub enum RuntimeResolveError {
  RuntimeCell(RuntimeCell),
  Eval(CalcitErr),
  RuntimeSeed(String),
}

pub type ProgramRuntimeData = Vec<RuntimeCell>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DefId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledDefKind {
  Proc,
  Fn,
  Macro,
  Syntax,
  LazyValue,
  Value,
}

impl CompiledDefKind {
  fn from_runtime_value(value: &Calcit) -> Self {
    match value {
      Calcit::Proc(..) => Self::Proc,
      Calcit::Fn { .. } => Self::Fn,
      Calcit::Macro { .. } => Self::Macro,
      Calcit::Syntax(..) => Self::Syntax,
      Calcit::Thunk(..) => Self::LazyValue,
      _ => Self::Value,
    }
  }

  fn from_preprocessed_code(code: &Calcit) -> Self {
    match code {
      Calcit::Proc(..) => Self::Proc,
      Calcit::Syntax(..) => Self::Syntax,
      Calcit::List(xs) => match xs.first() {
        Some(Calcit::Syntax(
          calcit::CalcitSyntax::Defn | calcit::CalcitSyntax::DefWasmExport | calcit::CalcitSyntax::DefWasmImport,
          _,
        )) => Self::Fn,
        Some(Calcit::Symbol { sym, .. }) if matches!(sym.as_ref(), "defn" | "defwasm-export" | "defwasm-import") => Self::Fn,
        Some(Calcit::Syntax(calcit::CalcitSyntax::Defmacro, _)) => Self::Macro,
        Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "defmacro" => Self::Macro,
        _ => Self::LazyValue,
      },
      _ => Self::LazyValue,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledDef {
  pub def_id: DefId,
  pub version_id: u32,
  pub kind: CompiledDefKind,
  pub preprocessed_code: Calcit,
  pub codegen_form: Calcit,
  pub deps: Vec<DefId>,
  pub type_summary: Option<Arc<str>>,
  pub source_code: Option<Calcit>,
  pub schema: Arc<CalcitTypeAnnotation>,
  pub doc: Arc<str>,
  pub examples: Vec<Cirru>,
}

pub struct CompiledDefPayload {
  pub version_id: u32,
  pub preprocessed_code: Calcit,
  pub codegen_form: Calcit,
  pub deps: Vec<DefId>,
  pub type_summary: Option<Arc<str>>,
  pub source_code: Option<Calcit>,
  pub schema: Arc<CalcitTypeAnnotation>,
  pub doc: Arc<str>,
  pub examples: Vec<Cirru>,
}

pub type ProgramCompiledData = HashMap<Arc<str>, CompiledFileData>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledFileData {
  pub defs: HashMap<Arc<str>, CompiledDef>,
}

impl CompiledFileData {
  pub fn keys(&self) -> impl Iterator<Item = &Arc<str>> {
    self.defs.keys()
  }

  pub fn get(&self, def: &str) -> Option<&CompiledDef> {
    self.defs.get(def)
  }
}

pub type CompiledProgram = HashMap<Arc<str>, CompiledFileData>;

#[derive(Debug, Default)]
struct ProgramDefIdIndex {
  next_id: u32,
  by_ns: HashMap<Arc<str>, HashMap<Arc<str>, DefId>>,
}

/// definition entry with code and documentation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramDefEntry {
  pub code: Calcit,
  pub schema: Arc<CalcitTypeAnnotation>,
  pub doc: Arc<str>,
  pub examples: Vec<Cirru>,
  pub ffi: Option<cirru_edn::Edn>,
}

/// information extracted from snapshot
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramFileData {
  pub import_map: HashMap<Arc<str>, Arc<ImportRule>>,
  pub defs: HashMap<Arc<str>, ProgramDefEntry>,
}

type ImportMapPair = (Arc<str>, Arc<ImportRule>);

/// defRule: ns def
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRule {
  /// ns imported via `:as`
  NsAs(Arc<str>),
  /// (ns, def) imported via `:refer`
  NsReferDef(Arc<str>, Arc<str>),
  /// ns imported via `:default`, js only
  NsDefault(Arc<str>),
}

pub type ProgramCodeData = HashMap<Arc<str>, ProgramFileData>;

/// runtime values keyed by stable DefId, used by normal runtime lookup paths
static PROGRAM_RUNTIME_DATA_STATE: LazyLock<RwLock<ProgramRuntimeData>> = LazyLock::new(|| RwLock::new(vec![]));
/// preprocessed / compiled definitions for codegen and future runtime boundary split
static PROGRAM_COMPILED_DATA_STATE: LazyLock<RwLock<ProgramCompiledData>> = LazyLock::new(|| RwLock::new(HashMap::new()));
/// raw code information before program running
pub static PROGRAM_CODE_DATA: LazyLock<RwLock<ProgramCodeData>> = LazyLock::new(|| RwLock::new(HashMap::new()));
static PROGRAM_DEF_ID_INDEX: LazyLock<RwLock<ProgramDefIdIndex>> = LazyLock::new(|| RwLock::new(ProgramDefIdIndex::default()));

fn ensure_runtime_capacity(runtime: &mut ProgramRuntimeData, def_id: DefId) {
  let idx = def_id.0 as usize;
  if runtime.len() <= idx {
    runtime.resize(idx + 1, RuntimeCell::Cold);
  }
}

fn register_program_def_id(ns: &str, def: &str) -> DefId {
  let mut index = PROGRAM_DEF_ID_INDEX.write().expect("write program def id index");
  if let Some(def_id) = index.by_ns.get(ns).and_then(|defs| defs.get(def)) {
    *def_id
  } else {
    let def_id = DefId(index.next_id);
    index.next_id += 1;
    index.by_ns.entry(Arc::from(ns)).or_default().insert(Arc::from(def), def_id);
    def_id
  }
}

pub fn ensure_def_id(ns: &str, def: &str) -> DefId {
  register_program_def_id(ns, def)
}

pub fn lookup_def_id(ns: &str, def: &str) -> Option<DefId> {
  let index = PROGRAM_DEF_ID_INDEX.read().expect("read program def id index");
  index.by_ns.get(ns).and_then(|defs| defs.get(def)).copied()
}

fn collect_ns_def_ids(ns: &str) -> Vec<DefId> {
  let index = PROGRAM_DEF_ID_INDEX.read().expect("read program def id index");
  index.by_ns.get(ns).map(|defs| defs.values().copied().collect()).unwrap_or_default()
}

fn write_runtime_cell(def_id: DefId, cell: RuntimeCell) {
  let mut runtime = PROGRAM_RUNTIME_DATA_STATE.write().expect("write runtime data");
  ensure_runtime_capacity(&mut runtime, def_id);
  runtime[def_id.0 as usize] = cell;
}

fn write_runtime_value(def_id: DefId, value: Calcit) {
  write_runtime_cell(def_id, RuntimeCell::Ready(value));
}

fn write_runtime_lazy(def_id: DefId, code: Arc<Calcit>, info: Arc<CalcitThunkInfo>) {
  write_runtime_cell(def_id, RuntimeCell::Lazy { code, info });
}

pub fn write_runtime_lazy_value(ns: &str, def: &str, code: Arc<Calcit>, info: Arc<CalcitThunkInfo>) {
  let def_id = ensure_def_id(ns, def);
  write_runtime_lazy(def_id, code, info);
}

fn clear_runtime_value(def_id: DefId) {
  let mut runtime = PROGRAM_RUNTIME_DATA_STATE.write().expect("write runtime data");
  if let Some(slot) = runtime.get_mut(def_id.0 as usize) {
    *slot = RuntimeCell::Cold;
  }
}

pub fn mark_runtime_def_cold(ns: &str, def: &str) {
  let def_id = ensure_def_id(ns, def);
  clear_runtime_value(def_id);
}

pub fn seed_runtime_lazy_from_compiled(ns: &str, def: &str) -> bool {
  let Some((def_id, preprocessed_code)) = with_compiled_def(ns, def, |compiled| {
    if compiled.kind == CompiledDefKind::LazyValue {
      Some((compiled.def_id, compiled.preprocessed_code.clone()))
    } else {
      None
    }
  })
  .flatten() else {
    return false;
  };

  match lookup_runtime_cell_by_id(def_id) {
    Some(RuntimeCell::Lazy { .. } | RuntimeCell::Ready(_) | RuntimeCell::Resolving | RuntimeCell::Errored(_)) => false,
    Some(RuntimeCell::Cold) | None => {
      write_runtime_lazy(
        def_id,
        Arc::new(preprocessed_code),
        Arc::new(CalcitThunkInfo {
          ns: Arc::from(ns),
          def: Arc::from(def),
        }),
      );
      true
    }
  }
}

fn clear_runtime_ns(ns: &str) {
  for def_id in collect_ns_def_ids(ns) {
    clear_runtime_value(def_id);
  }
}

pub fn lookup_runtime_cell_by_id(def_id: DefId) -> Option<RuntimeCell> {
  let runtime = PROGRAM_RUNTIME_DATA_STATE.read().expect("read runtime data");
  runtime.get(def_id.0 as usize).cloned()
}

pub fn lookup_runtime_cell(ns: &str, def: &str) -> Option<RuntimeCell> {
  lookup_def_id(ns, def).and_then(lookup_runtime_cell_by_id)
}

pub fn lookup_runtime_ready_by_id(def_id: DefId) -> Option<Calcit> {
  match lookup_runtime_cell_by_id(def_id)? {
    RuntimeCell::Ready(value) => Some(value),
    _ => None,
  }
}

pub fn lookup_runtime_ready(ns: &str, def: &str) -> Option<Calcit> {
  lookup_def_id(ns, def).and_then(lookup_runtime_ready_by_id)
}

pub fn mark_runtime_def_resolving(ns: &str, def: &str) {
  let def_id = ensure_def_id(ns, def);
  write_runtime_cell(def_id, RuntimeCell::Resolving);
}

pub fn mark_runtime_def_errored(ns: &str, def: &str, message: Arc<str>) {
  let def_id = ensure_def_id(ns, def);
  write_runtime_cell(def_id, RuntimeCell::Errored(message));
}

fn collect_compiled_dep_keys(code: &Calcit, deps: &mut Vec<(Arc<str>, Arc<str>)>) {
  match code {
    Calcit::Import(import) => deps.push((import.ns.to_owned(), import.def.to_owned())),
    Calcit::Thunk(thunk) => collect_compiled_dep_keys(thunk.get_code(), deps),
    Calcit::Fn { info, .. } => {
      for item in info.body.iter() {
        collect_compiled_dep_keys(item, deps);
      }
    }
    Calcit::List(xs) => {
      if matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::ParseCirruEdnAs, _)))
        && let Some(graph) = xs.get(3).and_then(DataShapeGraph::from_calcit_handle)
      {
        deps.extend(graph.nominal_paths());
      }
      for item in xs.iter() {
        collect_compiled_dep_keys(item, deps);
      }
    }
    _ => {}
  }
}

pub fn collect_compiled_deps(code: &Calcit) -> Vec<DefId> {
  let mut keys: Vec<(Arc<str>, Arc<str>)> = vec![];
  collect_compiled_dep_keys(code, &mut keys);

  let mut deps: Vec<DefId> = vec![];
  for (ns, def) in keys {
    if let Some(def_id) = lookup_def_id(&ns, &def)
      && !deps.contains(&def_id)
    {
      deps.push(def_id);
    }
  }
  deps.sort();
  deps
}

fn build_compiled_def(ns: &str, def: &str, payload: CompiledDefPayload) -> CompiledDef {
  let kind = CompiledDefKind::from_preprocessed_code(&payload.preprocessed_code);

  CompiledDef {
    def_id: ensure_def_id(ns, def),
    version_id: payload.version_id,
    kind,
    preprocessed_code: payload.preprocessed_code,
    codegen_form: payload.codegen_form,
    deps: payload.deps,
    type_summary: payload.type_summary,
    source_code: payload.source_code,
    schema: payload.schema,
    doc: payload.doc,
    examples: payload.examples,
  }
}

fn build_runtime_only_snapshot_fallback_compiled_def(ns: &str, def: &str, runtime_value: Calcit) -> Option<CompiledDef> {
  let codegen_form = data_to_calcit(&runtime_value, ns, def).ok()?;
  let kind = CompiledDefKind::from_runtime_value(&runtime_value);
  let deps = collect_compiled_deps(&codegen_form);

  Some(CompiledDef {
    def_id: ensure_def_id(ns, def),
    version_id: 0,
    kind,
    preprocessed_code: codegen_form.to_owned(),
    codegen_form,
    deps,
    type_summary: None,
    source_code: None,
    schema: DYNAMIC_TYPE.clone(),
    doc: Arc::from(""),
    examples: vec![],
  })
}

fn ensure_source_backed_compiled_def_for_snapshot(ns: &str, def: &str) -> Option<CompiledDef> {
  if let Some(compiled) = lookup_compiled_def(ns, def) {
    return Some(compiled);
  }

  if !has_def_code(ns, def) {
    return None;
  }

  let warnings: RefCell<Vec<calcit::LocatedWarning>> = RefCell::new(vec![]);
  if runner::preprocess::compile_source_def_for_snapshot(ns, def, &warnings, &CallStackList::default()).is_err() {
    return None;
  }

  lookup_compiled_def(ns, def)
}

pub fn write_compiled_def(ns: &str, def: &str, compiled: CompiledDef) {
  let mut program = PROGRAM_COMPILED_DATA_STATE.write().expect("write compiled program data");
  let file = program
    .entry(Arc::from(ns))
    .or_insert_with(|| CompiledFileData { defs: HashMap::new() });
  file.defs.insert(Arc::from(def), compiled);
}

pub fn store_compiled_output(ns: &str, def: &str, payload: CompiledDefPayload) {
  let compiled = build_compiled_def(ns, def, payload);
  write_compiled_def(ns, def, compiled);
}

pub fn lookup_compiled_def(ns: &str, def: &str) -> Option<CompiledDef> {
  let program = PROGRAM_COMPILED_DATA_STATE.read().expect("read compiled program data");
  let file = program.get(ns)?;
  file.defs.get(def).cloned()
}

fn with_compiled_def<T>(ns: &str, def: &str, f: impl FnOnce(&CompiledDef) -> T) -> Option<T> {
  let program = PROGRAM_COMPILED_DATA_STATE.read().expect("read compiled program data");
  let file = program.get(ns)?;
  let compiled = file.defs.get(def)?;
  Some(f(compiled))
}

fn is_compiled_executable_kind(kind: &CompiledDefKind) -> bool {
  matches!(
    kind,
    CompiledDefKind::Fn | CompiledDefKind::Macro | CompiledDefKind::Proc | CompiledDefKind::Syntax
  )
}

#[cfg(test)]
fn lookup_compiled_executable_code(ns: &str, def: &str) -> Option<Calcit> {
  with_compiled_def(ns, def, |compiled| {
    if is_compiled_executable_kind(&compiled.kind) {
      Some(compiled.preprocessed_code.to_owned())
    } else {
      None
    }
  })
  .flatten()
}

fn materialize_compiled_executable_payload(
  ns: &str,
  def: &str,
  call_stack: &CallStackList,
) -> Result<Option<Calcit>, RuntimeResolveError> {
  let Some(result) = with_compiled_def(ns, def, |compiled| {
    if !is_compiled_executable_kind(&compiled.kind) {
      return None;
    }

    match compiled.kind {
      CompiledDefKind::Proc | CompiledDefKind::Syntax => Some(Ok(compiled.preprocessed_code.to_owned())),
      CompiledDefKind::Fn | CompiledDefKind::Macro => Some(runner::evaluate_expr(
        &compiled.preprocessed_code,
        &CalcitScope::default(),
        ns,
        call_stack,
      )),
      CompiledDefKind::LazyValue | CompiledDefKind::Value => None,
    }
  })
  .flatten() else {
    return Ok(None);
  };

  let value = result.map_err(RuntimeResolveError::Eval)?;
  write_runtime_ready(ns, def, value.to_owned()).map_err(RuntimeResolveError::RuntimeSeed)?;
  Ok(Some(value))
}

pub fn resolve_compiled_executable_def(ns: &str, def: &str, call_stack: &CallStackList) -> Result<Option<Calcit>, RuntimeResolveError> {
  materialize_compiled_executable_payload(ns, def, call_stack)
}

fn resolve_runtime_ready_value(value: Calcit, call_stack: &CallStackList) -> Result<Calcit, RuntimeResolveError> {
  match value {
    Calcit::Thunk(thunk) => thunk.evaluated_default(call_stack).map_err(RuntimeResolveError::Eval),
    _ => Ok(value),
  }
}

fn resolve_runtime_cell_value(
  cell: RuntimeCell,
  mode: RuntimeResolveMode,
  call_stack: &CallStackList,
) -> Result<Option<Calcit>, RuntimeResolveError> {
  match cell {
    RuntimeCell::Lazy { code, info } => CalcitThunk::Code { code, info }
      .evaluated_default(call_stack)
      .map(Some)
      .map_err(RuntimeResolveError::Eval),
    RuntimeCell::Ready(value) => resolve_runtime_ready_value(value, call_stack).map(Some),
    RuntimeCell::Resolving | RuntimeCell::Errored(_) => {
      if matches!(mode, RuntimeResolveMode::Strict) {
        Err(RuntimeResolveError::RuntimeCell(cell))
      } else {
        Ok(None)
      }
    }
    RuntimeCell::Cold => Ok(None),
  }
}

fn resolve_runtime_def_value(
  ns: &str,
  def: &str,
  def_id: Option<DefId>,
  mode: RuntimeResolveMode,
  call_stack: &CallStackList,
) -> Result<Option<Calcit>, RuntimeResolveError> {
  let runtime_def_id = def_id.or_else(|| lookup_def_id(ns, def));

  if let Some(def_id) = runtime_def_id {
    match lookup_runtime_cell_by_id(def_id) {
      Some(RuntimeCell::Cold) | None => {
        let _ = seed_runtime_lazy_from_compiled(ns, def);
        // Re-read only after seeding changed the cell
        if let Some(cell) = lookup_runtime_cell_by_id(def_id) {
          return resolve_runtime_cell_value(cell, mode, call_stack);
        }
      }
      Some(cell) => {
        // Fast path: reuse the already-read cell (avoids redundant RwLock read)
        return resolve_runtime_cell_value(cell, mode, call_stack);
      }
    }
  }

  Ok(None)
}

pub fn resolve_runtime_or_compiled_def(
  ns: &str,
  def: &str,
  def_id: Option<DefId>,
  mode: RuntimeResolveMode,
  call_stack: &CallStackList,
) -> Result<Option<Calcit>, RuntimeResolveError> {
  if let Some(value) = resolve_runtime_def_value(ns, def, def_id, mode, call_stack)? {
    return Ok(Some(value));
  }

  materialize_compiled_executable_payload(ns, def, call_stack)
}

fn annotation_from_value(value: &Calcit) -> Option<Arc<CalcitTypeAnnotation>> {
  let annotation = Arc::new(calcit::CalcitTypeAnnotation::from_calcit(value));
  if matches!(annotation.as_ref(), CalcitTypeAnnotation::Dynamic) {
    None
  } else {
    Some(annotation)
  }
}

pub fn lookup_codegen_type_hint(ns: &str, def: &str) -> Option<Arc<CalcitTypeAnnotation>> {
  if let Some(schema) = with_compiled_def(ns, def, |compiled| compiled.schema.clone())
    && !matches!(schema.as_ref(), CalcitTypeAnnotation::Dynamic)
  {
    return Some(schema);
  }

  let source_schema = lookup_def_schema(ns, def);
  if !matches!(source_schema.as_ref(), CalcitTypeAnnotation::Dynamic) {
    return Some(source_schema);
  }

  lookup_runtime_ready(ns, def).as_ref().and_then(annotation_from_value)
}

fn remove_compiled_def(ns: &str, def: &str) {
  let mut program = PROGRAM_COMPILED_DATA_STATE.write().expect("write compiled program data");
  if let Some(file) = program.get_mut(ns) {
    file.defs.remove(def);
    if file.defs.is_empty() {
      program.remove(ns);
    }
  }
}

fn remove_compiled_ns(ns: &str) {
  let mut program = PROGRAM_COMPILED_DATA_STATE.write().expect("write compiled program data");
  program.remove(ns);
}

fn collect_transitive_dependent_def_ids(compiled: &ProgramCompiledData, seed_ids: &HashSet<DefId>) -> HashSet<DefId> {
  if seed_ids.is_empty() {
    return HashSet::new();
  }

  let mut reverse_deps: HashMap<DefId, Vec<DefId>> = HashMap::new();
  for file in compiled.values() {
    for compiled_def in file.defs.values() {
      for dep in &compiled_def.deps {
        reverse_deps.entry(*dep).or_default().push(compiled_def.def_id);
      }
    }
  }

  let mut affected = seed_ids.clone();
  let mut pending: VecDeque<DefId> = seed_ids.iter().copied().collect();

  while let Some(def_id) = pending.pop_front() {
    if let Some(dependents) = reverse_deps.get(&def_id) {
      for dependent_id in dependents {
        if affected.insert(*dependent_id) {
          pending.push_back(*dependent_id);
        }
      }
    }
  }

  affected
}

fn collect_changed_seed_def_ids(changes: &snapshot::ChangesDict, index: &ProgramDefIdIndex) -> HashSet<DefId> {
  let mut seeds: HashSet<DefId> = HashSet::new();

  for ns in &changes.removed {
    if let Some(defs) = index.by_ns.get(ns) {
      seeds.extend(defs.values().copied());
    }
  }

  for (ns, info) in &changes.changed {
    if info.ns.is_some()
      && let Some(defs) = index.by_ns.get(ns)
    {
      seeds.extend(defs.values().copied());
    }

    for def in info.removed_defs.iter() {
      if let Some(def_id) = index.by_ns.get(ns).and_then(|defs| defs.get(def.as_str())) {
        seeds.insert(*def_id);
      }
    }

    for def in info.changed_defs.keys() {
      if let Some(def_id) = index.by_ns.get(ns).and_then(|defs| defs.get(def.as_str())) {
        seeds.insert(*def_id);
      }
    }
  }

  seeds
}

fn collect_reload_affected_def_ids(
  changes: &snapshot::ChangesDict,
  compiled: &ProgramCompiledData,
  index: &ProgramDefIdIndex,
) -> HashSet<DefId> {
  let seed_ids = collect_changed_seed_def_ids(changes, index);
  collect_transitive_dependent_def_ids(compiled, &seed_ids)
}

fn register_program_def_ids(program_data: &ProgramCodeData) {
  for (ns, file) in program_data {
    for def in file.defs.keys() {
      let _ = register_program_def_id(ns, def);
    }
  }
}

fn extract_import_rule(nodes: &Cirru) -> Result<Vec<ImportMapPair>, String> {
  match nodes {
    Cirru::Leaf(_) => Err(format!(
      "expected an import rule list, got {nodes}; expected `(namespace :as alias)`, `(namespace :refer (definition ...))`, or `(module :default alias)`"
    )),
    Cirru::List(rule_nodes) => {
      let xs = match rule_nodes.first() {
        // strip leading `[]` symbols
        Some(Cirru::Leaf(s)) if &**s == "[]" => &rule_nodes[1..],
        // allow using comment
        Some(Cirru::Leaf(s)) if &**s == ";" => return Ok(vec![]),
        _ => rule_nodes.as_slice(),
      };
      if xs.len() != 3 {
        return Err(format!(
          "expected import rule to contain exactly 3 items, got {} in {nodes}; expected `(namespace :as alias)`, `(namespace :refer (definition ...))`, or `(module :default alias)`",
          xs.len()
        ));
      }
      match (&xs[0], &xs[1], &xs[2]) {
        (Cirru::Leaf(ns), x, Cirru::Leaf(alias)) if x.eq_leaf(":as") => Ok(vec![(
          (*alias.to_owned()).into(),
          Arc::new(ImportRule::NsAs((*ns.to_owned()).into())),
        )]),
        (Cirru::Leaf(ns), x, Cirru::Leaf(alias)) if x.eq_leaf(":default") => Ok(vec![(
          (*alias.to_owned()).into(),
          Arc::new(ImportRule::NsDefault((*ns.to_owned()).into())),
        )]),
        (Cirru::Leaf(ns), x, Cirru::List(ys)) if x.eq_leaf(":refer") => {
          let mut rules: Vec<(Arc<str>, Arc<ImportRule>)> = Vec::with_capacity(ys.len());
          for y in ys {
            match y {
              Cirru::Leaf(s) if &**s == "[]" => (), // `[]` symbol are ignored
              Cirru::Leaf(s) => rules.push((
                (*s.to_owned()).into(),
                Arc::new(ImportRule::NsReferDef((*ns.to_owned()).into(), (*s.to_owned()).into())),
              )),
              Cirru::List(_defs) => return Err(format!("invalid refer values, {y}")),
            }
          }
          Ok(rules)
        }
        (_, x, _) if x.eq_leaf(":as") => Err(format!("invalid `:as` import rule: {nodes}; namespace and alias must be symbols")),
        (_, x, _) if x.eq_leaf(":default") => Err(format!("invalid `:default` import rule: {nodes}; module and alias must be symbols")),
        (_, x, _) if x.eq_leaf(":refer") => Err(format!(
          "invalid `:refer` import rule: {nodes}; namespace must be a symbol and referred definitions must be a list"
        )),
        (_, Cirru::Leaf(kind), _) => Err(format!(
          "unknown import rule kind `{kind}` in {nodes}; expected `:as`, `:refer`, or `:default`"
        )),
        _ => Err(format!("invalid import rule: {nodes}; rule kind must be a symbol")),
      }
    }
  }
}

/// Validate import rules before they are persisted or loaded into a program.
/// Malformed rules return an error; duplicate local bindings return warnings and keep last-rule-wins semantics.
pub fn validate_import_rules(rules: &[Cirru]) -> Result<Vec<String>, String> {
  let mut bindings: HashMap<Arc<str>, usize> = HashMap::new();
  let mut warnings = vec![];
  for (index, rule) in rules.iter().enumerate() {
    for (binding, _) in extract_import_rule(rule).map_err(|error| format!("import rule {}: {error}", index + 1))? {
      if let Some(previous_index) = bindings.insert(binding.clone(), index) {
        if previous_index == index {
          warnings.push(format!(
            "duplicate import binding `{binding}` within rule {}; the later `:refer` entry takes precedence",
            index + 1
          ));
        } else {
          warnings.push(format!(
            "duplicate import binding `{binding}` in rules {} and {}; rule {} takes precedence",
            previous_index + 1,
            index + 1,
            index + 1
          ));
        }
      }
    }
  }
  Ok(warnings)
}

fn extract_import_map(nodes: &Cirru, ns_name: &str) -> Result<HashMap<Arc<str>, Arc<ImportRule>>, String> {
  match nodes {
    Cirru::List(xs) => match xs.as_slice() {
      [head, Cirru::Leaf(_declared_ns)] if head.eq_leaf("ns") || head.eq_leaf(":ns") => Ok(HashMap::new()),
      [head, Cirru::Leaf(_declared_ns), Cirru::List(require_nodes)] if head.eq_leaf("ns") || head.eq_leaf(":ns") => {
        if require_nodes.first().is_none_or(|node| !node.eq_leaf(":require")) {
          return Err(format!(
            "invalid ns clause in namespace '{ns_name}': expected `:require`, got {}",
            Cirru::List(require_nodes.clone())
          ));
        }

        let rules = &require_nodes[1..];
        let warnings = validate_import_rules(rules).map_err(|e| format!("in namespace '{ns_name}': {e}"))?;
        for warning in warnings {
          eprintln!("[Warn] in namespace '{ns_name}': {warning}");
        }
        let mut import_map: HashMap<Arc<str>, Arc<ImportRule>> = HashMap::with_capacity(rules.len());
        for rule_node in rules {
          for (target, rule) in extract_import_rule(rule_node).map_err(|e| format!("in namespace '{ns_name}': {e}"))? {
            import_map.insert(target, rule);
          }
        }
        Ok(import_map)
      }
      _ => {
        let preview = cirru_parser::format(std::slice::from_ref(nodes), true.into()).unwrap_or_else(|_| format!("{nodes:?}"));
        let preview_short = if preview.chars().count() > 200 {
          format!("{}...", preview.chars().take(200).collect::<String>())
        } else {
          preview
        };
        Err(format!("invalid ns form in '{ns_name}':\n{preview_short}"))
      }
    },
    Cirru::Leaf(_) => Err(format!(
      "invalid ns form in '{ns_name}': expected `(ns {ns_name})` (legacy `:ns` is also accepted) with an optional `:require` clause, got {nodes}"
    )),
  }
}

fn extract_file_data(file: &snapshot::FileInSnapShot, ns: Arc<str>) -> Result<ProgramFileData, String> {
  let import_map = extract_import_map(&file.ns.code, &ns)?;
  let mut defs: HashMap<Arc<str>, ProgramDefEntry> = HashMap::with_capacity(file.defs.len());
  for (def, entry) in &file.defs {
    let at_def = def.to_owned();
    let code = code_to_calcit(&entry.code, &ns, &at_def, vec![])?;
    let schema = entry.schema.clone();
    let doc = Arc::from(entry.doc.as_str());
    defs.insert(
      def.to_owned().into(),
      ProgramDefEntry {
        code,
        schema,
        doc,
        examples: entry.examples.clone(),
        ffi: entry.ffi.clone(),
      },
    );
  }
  Ok(ProgramFileData { import_map, defs })
}

pub fn extract_program_data(s: &Snapshot) -> Result<ProgramCodeData, String> {
  // Register the program lookup functions in type_annotation so it can resolve
  // imported type definitions without a circular module dependency.
  calcit::register_program_lookups(lookup_runtime_ready, lookup_def_code, lookup_def_schema);
  calcit::clear_type_slots();

  let entry = s.active_entry()?;
  let entry_owner = format!("entries.{}", s.active_entry_name());

  for (slot, type_path) in &entry.type_slots {
    if matches!(type_path.as_str(), ":dynamic" | "dynamic") {
      continue;
    }
    let Some((ns, def)) = type_path.rsplit_once('/') else {
      return Err(format!(
        "{entry_owner}.type-slots.{slot}: expected a full `namespace/definition` type path, got `{type_path}`"
      ));
    };
    if !s.files.get(ns).is_some_and(|file| file.defs.contains_key(def)) {
      return Err(format!(
        "{entry_owner}.type-slots.{slot}: unknown type definition `{type_path}`; load its module and use a full `namespace/definition` path"
      ));
    }
  }
  calcit::configure_entry_type_slots(&entry.type_slots).map_err(|message| format!("{entry_owner}.type-slots: {message}"))?;

  let mut xs: ProgramCodeData = HashMap::with_capacity(s.files.len());

  for (ns, file) in &s.files {
    let file_info = extract_file_data(file, ns.to_owned().into())?;
    xs.insert(ns.to_owned().into(), file_info);
  }

  register_program_def_ids(&xs);

  Ok(xs)
}

// lookup without cloning
pub fn has_def_code(ns: &str, def: &str) -> bool {
  let program_code = { PROGRAM_CODE_DATA.read().expect("read program code") };
  match &program_code.get(ns) {
    Some(v) => v.defs.contains_key(def),
    None => false,
  }
}

/// List all def names in a source-code namespace.
pub fn list_source_def_names(ns: &str) -> Vec<Arc<str>> {
  let program_code = PROGRAM_CODE_DATA.read().expect("read program code");
  match program_code.get(ns) {
    Some(file) => file.defs.keys().cloned().collect(),
    None => vec![],
  }
}

pub fn lookup_def_code(ns: &str, def: &str) -> Option<Calcit> {
  let program_code = { PROGRAM_CODE_DATA.read().expect("read program code") };
  let file = program_code.get(ns)?;
  let entry = file.defs.get(def)?;
  Some(entry.code.to_owned())
}

pub fn lookup_def_schema(ns: &str, def: &str) -> Arc<CalcitTypeAnnotation> {
  let program_code = { PROGRAM_CODE_DATA.read().expect("read program code") };
  let file = match program_code.get(ns) {
    Some(f) => f,
    None => return DYNAMIC_TYPE.clone(),
  };
  let entry = match file.defs.get(def) {
    Some(e) => e,
    None => return DYNAMIC_TYPE.clone(),
  };
  entry.schema.clone()
}

/// lookup documentation for a definition from program data
pub fn lookup_def_doc(ns: &str, def: &str) -> Option<String> {
  let program_code = { PROGRAM_CODE_DATA.read().expect("read program code") };
  let file = program_code.get(ns)?;
  let entry = file.defs.get(def)?;
  if entry.doc.is_empty() { None } else { Some(entry.doc.to_string()) }
}

/// lookup examples for a definition from program data
pub fn lookup_def_examples(ns: &str, def: &str) -> Option<Vec<Cirru>> {
  let program_code = { PROGRAM_CODE_DATA.read().expect("read program code") };
  let file = program_code.get(ns)?;
  let entry = file.defs.get(def)?;
  if entry.examples.is_empty() {
    None
  } else {
    Some(entry.examples.clone())
  }
}

/// Lookup optional host/FFI metadata attached to a definition's CodeEntry.
pub fn lookup_def_ffi(ns: &str, def: &str) -> Option<cirru_edn::Edn> {
  let program_code = PROGRAM_CODE_DATA.read().expect("read program code");
  let file = program_code.get(ns)?;
  file.defs.get(def)?.ffi.clone()
}

pub fn lookup_def_target_in_import(ns: &str, def: &str) -> Option<Arc<str>> {
  let program = { PROGRAM_CODE_DATA.read().expect("read program code") };
  let file = program.get(ns)?;
  let import_rule = file.import_map.get(def)?;
  match &**import_rule {
    ImportRule::NsReferDef(ns, _def) => Some(ns.to_owned()),
    ImportRule::NsAs(_ns) => None,
    ImportRule::NsDefault(_ns) => None,
  }
}

pub fn lookup_ns_target_in_import(ns: &str, alias: &str) -> Option<Arc<str>> {
  let program = { PROGRAM_CODE_DATA.read().expect("read program code") };
  let file = program.get(ns)?;
  let import_rule = file.import_map.get(alias)?;
  match &**import_rule {
    ImportRule::NsReferDef(_ns, _def) => None,
    ImportRule::NsAs(ns) => Some(ns.to_owned()),
    ImportRule::NsDefault(_ns) => None,
  }
}

// imported via :default
pub fn lookup_default_target_in_import(at_ns: &str, alias: &str) -> Option<Arc<str>> {
  let program = { PROGRAM_CODE_DATA.read().expect("read program code") };
  let file = program.get(at_ns)?;
  let import_rule = file.import_map.get(alias)?;
  match &**import_rule {
    ImportRule::NsReferDef(_ns, _def) => None,
    ImportRule::NsAs(_ns) => None,
    ImportRule::NsDefault(ns) => Some(ns.to_owned()),
  }
}

// Dirty mutating global states
pub fn write_runtime_ready(ns: &str, def: &str, value: Calcit) -> Result<(), String> {
  let def_id = ensure_def_id(ns, def);

  match value {
    Calcit::Thunk(CalcitThunk::Code { code, info }) => write_runtime_lazy(def_id, code, info),
    Calcit::Trait(trait_def) if trait_def.definition_ref.is_none() => {
      write_runtime_value(def_id, Calcit::Trait(trait_def.with_definition_ref(ns, def)))
    }
    other => write_runtime_value(def_id, other),
  }

  Ok(())
}

#[derive(Debug, Clone)]
struct SnapshotFillTask {
  ns: Arc<str>,
  def: Arc<str>,
  source_backed: bool,
  referenced_by_compiled: bool,
  runtime_value: Option<Calcit>,
}

fn collect_snapshot_fill_tasks(compiled: &CompiledProgram) -> Vec<SnapshotFillTask> {
  let program_code = PROGRAM_CODE_DATA.read().expect("read program code");
  let program_def_ids = PROGRAM_DEF_ID_INDEX.read().expect("read program def id index");
  let runtime = PROGRAM_RUNTIME_DATA_STATE.read().expect("read runtime data");
  let mut referenced_def_ids: HashSet<DefId> = HashSet::new();
  for file in compiled.values() {
    for compiled_def in file.defs.values() {
      referenced_def_ids.extend(compiled_def.deps.iter().copied());
    }
  }

  let mut tasks = vec![];

  for (ns, defs) in &program_def_ids.by_ns {
    let existing_defs = compiled.get(ns).map(|file| &file.defs);
    let source_file = program_code.get(ns.as_ref());

    for (def, def_id) in defs {
      if existing_defs.is_some_and(|defs| defs.contains_key(def.as_ref())) {
        continue;
      }

      let source_backed = source_file.is_some_and(|data| data.defs.contains_key(def.as_ref()));
      let referenced_by_compiled = referenced_def_ids.contains(def_id);
      let runtime_value = runtime.get(def_id.0 as usize).and_then(|cell| match cell {
        RuntimeCell::Ready(value) => Some(value.to_owned()),
        _ => None,
      });

      if !source_backed && (!referenced_by_compiled || runtime_value.is_none()) {
        continue;
      }

      tasks.push(SnapshotFillTask {
        ns: ns.to_owned(),
        def: def.to_owned(),
        source_backed,
        referenced_by_compiled,
        runtime_value,
      });
    }
  }

  tasks.sort_by(|a, b| a.ns.cmp(&b.ns).then_with(|| a.def.cmp(&b.def)));

  tasks
}

fn build_snapshot_fill_compiled_def(task: SnapshotFillTask) -> Option<CompiledDef> {
  if task.source_backed {
    return ensure_source_backed_compiled_def_for_snapshot(task.ns.as_ref(), task.def.as_ref());
  }

  if !task.source_backed
    && task.referenced_by_compiled
    && task.runtime_value.is_some()
    && let Some(runtime_value) = task.runtime_value
  {
    return build_runtime_only_snapshot_fallback_compiled_def(task.ns.as_ref(), task.def.as_ref(), runtime_value);
  }

  None
}

pub fn clone_compiled_program_snapshot() -> Result<CompiledProgram, String> {
  let mut compiled: CompiledProgram = PROGRAM_COMPILED_DATA_STATE.read().expect("read compiled program data").to_owned();
  let tasks = collect_snapshot_fill_tasks(&compiled);

  for task in tasks {
    let ns = task.ns.clone();
    let def = task.def.clone();
    if let Some(compiled_def) = build_snapshot_fill_compiled_def(task) {
      let compiled_file = compiled.entry(ns).or_insert_with(|| CompiledFileData { defs: HashMap::new() });
      compiled_file.defs.insert(def, compiled_def);
    }
  }

  Ok(compiled)
}

pub fn apply_code_changes(changes: &snapshot::ChangesDict) -> Result<(), String> {
  let mut program_code = PROGRAM_CODE_DATA.write().expect("open program code");
  let coord0 = vec![];

  for (ns, file) in &changes.added {
    let file_info = extract_file_data(file, ns.to_owned())?;
    for def in file_info.defs.keys() {
      let _ = register_program_def_id(ns, def);
    }
    program_code.insert(ns.to_owned(), file_info);
    remove_compiled_ns(ns);
  }
  for ns in &changes.removed {
    clear_runtime_ns(ns);
    program_code.remove(ns);
    remove_compiled_ns(ns);
  }
  for (ns, info) in &changes.changed {
    // println!("handling ns: {:?} {}", ns, program_code.contains_key(ns));
    let file = program_code.get_mut(ns).ok_or_else(|| format!("can not load ns: {ns}"))?;
    if let Some(v) = &info.ns {
      file.import_map = extract_import_map(v, ns)?;
    }
    for (def, code) in &info.added_defs {
      let _ = register_program_def_id(ns, def);
      clear_runtime_value(ensure_def_id(ns, def));
      remove_compiled_def(ns, def);
      let calcit_code = code_to_calcit(code, ns, def, coord0.to_owned())?;
      let entry = ProgramDefEntry {
        code: calcit_code,
        schema: DYNAMIC_TYPE.clone(),
        doc: Arc::from(""), // No doc info in changes, use empty string
        examples: vec![],   // No examples info in changes, use empty vector
        ffi: None,
      };
      file.defs.insert(def.to_owned().into(), entry);
    }
    for def in &info.removed_defs {
      if let Some(def_id) = lookup_def_id(ns, def) {
        clear_runtime_value(def_id);
      }
      file.defs.remove(def.as_str());
      remove_compiled_def(ns, def);
    }
    for (def, code) in &info.changed_defs {
      let def_id = register_program_def_id(ns, def);
      clear_runtime_value(def_id);
      remove_compiled_def(ns, def);
      let calcit_code = code_to_calcit(code, ns, def, coord0.to_owned())?;
      let (schema, doc, examples, ffi) = match file.defs.get(def.as_str()) {
        Some(existing) => (
          existing.schema.clone(),
          existing.doc.clone(),
          existing.examples.clone(),
          existing.ffi.clone(),
        ),
        None => (DYNAMIC_TYPE.clone(), Arc::from(""), Vec::new(), None),
      };
      file.defs.insert(
        def.to_owned().into(),
        ProgramDefEntry {
          code: calcit_code,
          schema,
          doc,
          examples,
          ffi,
        },
      );
    }
  }

  Ok(())
}

/// clear runtime and compiled caches after reloading
pub fn clear_runtime_caches_for_reload(init_ns: Arc<str>, reload_ns: Arc<str>, reload_libs: bool) -> Result<(), String> {
  if reload_libs {
    let mut runtime = PROGRAM_RUNTIME_DATA_STATE.write().expect("open runtime data");
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("open compiled program data");
    runtime.clear();
    compiled.clear();
    return Ok(());
  }

  let init_pkg = extract_pkg_from_ns(init_ns.to_owned()).ok_or_else(|| format!("failed to extract pkg from: {init_ns}"))?;
  let reload_pkg = extract_pkg_from_ns(reload_ns.to_owned()).ok_or_else(|| format!("failed to extract pkg from: {reload_ns}"))?;
  let ns_keys: Vec<Arc<str>> = {
    let index = PROGRAM_DEF_ID_INDEX.read().expect("read program def id index");
    index.by_ns.keys().cloned().collect()
  };

  let mut changes = snapshot::ChangesDict::default();
  for ns in ns_keys {
    if ns == init_pkg || ns == reload_pkg || ns.starts_with(&format!("{init_pkg}.")) || ns.starts_with(&format!("{reload_pkg}.")) {
      changes.changed.insert(
        ns,
        snapshot::FileChangeInfo {
          ns: Some(Cirru::Leaf(Arc::from("ns"))),
          added_defs: HashMap::new(),
          removed_defs: HashSet::new(),
          changed_defs: HashMap::new(),
        },
      );
    }
  }

  clear_runtime_caches_for_changes(&changes, false)
}

pub fn clear_runtime_caches_for_changes(changes: &snapshot::ChangesDict, reload_libs: bool) -> Result<(), String> {
  if reload_libs {
    let mut runtime = PROGRAM_RUNTIME_DATA_STATE.write().expect("open runtime data");
    let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("open compiled program data");
    runtime.clear();
    compiled.clear();
    return Ok(());
  }

  let affected_def_ids = {
    let compiled = PROGRAM_COMPILED_DATA_STATE.read().expect("read compiled program data");
    let index = PROGRAM_DEF_ID_INDEX.read().expect("read program def id index");
    collect_reload_affected_def_ids(changes, &compiled, &index)
  };

  if affected_def_ids.is_empty() {
    return Ok(());
  }

  let mut runtime = PROGRAM_RUNTIME_DATA_STATE.write().expect("open runtime data");
  for def_id in &affected_def_ids {
    if let Some(slot) = runtime.get_mut(def_id.0 as usize) {
      *slot = RuntimeCell::Cold;
    }
  }

  let mut compiled = PROGRAM_COMPILED_DATA_STATE.write().expect("open compiled program data");
  for file in compiled.values_mut() {
    file.defs.retain(|_, compiled_def| !affected_def_ids.contains(&compiled_def.def_id));
  }
  compiled.retain(|_, file| !file.defs.is_empty());

  Ok(())
}
