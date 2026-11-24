//! Pre-codegen reachability closure, tag indexing, and static pool emission.
//!
//! Computes the set of reachable compiled definitions from WASM/WASI entry
//! points and explicit exports so unreachable `calcit.core` functions, nominal
//! descriptors, tags, and static strings do not inflate emitted binaries.

use super::*;
use std::collections::{BTreeSet, HashMap, HashSet};

/// Builtin type tags always registered in `tag_index` so `type-of` can return
/// them and heap objects can carry them in their header slot.
const BUILTIN_TYPE_TAGS: &[&str] = &[
  "buf-list",
  "list",
  "map",
  "set",
  "enum",
  "struct",
  "number",
  "bool",
  "nil",
  "unit",
  "tag",
  "fn",
  "string",
  "symbol",
  "buffer",
  "none",
  "some",
  "ok",
  "err",
  "total-limit",
];

const CORE_BUILTIN_IMPL_DEFS: &[&str] = &[
  "&core-text-scalar-impls",
  "&core-list-impls",
  "&core-string-impls",
  "&core-map-impls",
  "&core-set-impls",
  "&core-ref-impls",
  "&core-number-impls",
  "&core-fn-impls",
  "&core-cirru-quote-impls",
  "&core-struct-impls",
  "&core-enum-impls",
  "&core-scalar-impls",
];

pub(super) struct ReachableProgram {
  pub(super) defs: HashSet<(String, String)>,
  pub(super) extra_exprs: Vec<Calcit>,
}

pub(super) struct ReachabilityConfig<'a> {
  pub(super) init_ns: &'a str,
  pub(super) init_def: &'a str,
  pub(super) target: WasmTarget,
  pub(super) extra_imports: &'a [ComponentImportAdapter],
  pub(super) wasi_command_component: bool,
  pub(super) wasi_http_proxy: bool,
  pub(super) wasi_command_read_dir: bool,
}

#[derive(Clone, Copy, Default)]
pub(super) struct StaticPoolOptions {
  pub(super) target: WasmTarget,
  pub(super) wasi_stdio: bool,
  pub(super) wasi_http_proxy: bool,
  pub(super) wasi_read_dir: bool,
  pub(super) needs_json_parse: bool,
  pub(super) needs_value_formatting: bool,
}

struct ReachabilityState<'a> {
  program_data: &'a program::CompiledProgram,
  def_id_to_name: HashMap<program::DefId, (String, String)>,
  core_builtin_impls: Vec<Arc<crate::wasm::host::calcit::CalcitImpl>>,
  visited: HashSet<(String, String)>,
  pending: Vec<(String, String)>,
  called_methods: BTreeSet<String>,
  wildcard_methods: bool,
  seen_impls: Vec<Arc<crate::wasm::host::calcit::CalcitImpl>>,
  seen_impl_ptrs: HashSet<usize>,
  visited_annotations: HashSet<(usize, String)>,
  retained_annotations: Vec<Arc<CalcitTypeAnnotation>>,
  active_alias_expansions: Vec<(Arc<str>, Vec<CalcitTypeAnnotation>)>,
  extra_exprs: Vec<Calcit>,
}

impl<'a> ReachabilityState<'a> {
  fn new(program_data: &'a program::CompiledProgram) -> Self {
    let mut def_id_to_name = HashMap::new();
    for (ns, file) in program_data {
      for (name, compiled) in &file.defs {
        def_id_to_name.insert(compiled.def_id, (ns.to_string(), name.to_string()));
      }
    }
    let mut core_builtin_impls = Vec::new();
    for def_name in CORE_BUILTIN_IMPL_DEFS {
      if let Some(value) = program::lookup_runtime_ready(crate::wasm::host::calcit::CORE_NS, def_name) {
        collect_impls_into(&value, &mut core_builtin_impls);
      }
    }
    Self {
      program_data,
      def_id_to_name,
      core_builtin_impls,
      visited: HashSet::new(),
      pending: Vec::new(),
      called_methods: BTreeSet::new(),
      wildcard_methods: false,
      seen_impls: Vec::new(),
      seen_impl_ptrs: HashSet::new(),
      visited_annotations: HashSet::new(),
      retained_annotations: Vec::new(),
      active_alias_expansions: Vec::new(),
      extra_exprs: Vec::new(),
    }
  }

  fn enqueue(&mut self, ns: &str, name: &str) {
    let key = (ns.to_owned(), name.to_owned());
    if !self.visited.contains(&key) && self.program_data.get(ns).is_some_and(|file| file.defs.contains_key(name)) {
      self.pending.push(key);
    }
  }

  fn enqueue_qualified(&mut self, qualified: &str) {
    if let Some((ns, name)) = qualified.rsplit_once('/') {
      self.enqueue(ns, name);
    }
  }

  fn enqueue_type_ref(&mut self, current_ns: &str, name: &str) {
    let trimmed = name.trim_start_matches('\'').trim_start_matches(':');
    if let Some((ns, def_name)) = program::resolve_type_name_in_ns(current_ns, trimmed) {
      self.enqueue(ns.as_ref(), def_name.as_ref());
    }
    if let Some((ns, def_name)) = trimmed.rsplit_once('/') {
      self.enqueue(ns, def_name);
    } else if !current_ns.is_empty() {
      self.enqueue(current_ns, trimmed);
    }
  }

  fn record_impl(&mut self, imp: &Arc<crate::wasm::host::calcit::CalcitImpl>, current_ns: &str) {
    let ptr = Arc::as_ptr(imp) as usize;
    if !self.seen_impl_ptrs.insert(ptr) {
      return;
    }
    self.seen_impls.push(imp.clone());
    if self.wildcard_methods {
      let values: Vec<Calcit> = imp.values.iter().cloned().collect();
      for val in values {
        self.extra_exprs.push(val.clone());
        self.visit_expr(&val, current_ns, false);
      }
    } else {
      let matching: Vec<Calcit> = self.called_methods.iter().filter_map(|method| imp.get(method).cloned()).collect();
      for val in matching {
        self.extra_exprs.push(val.clone());
        self.visit_expr(&val, current_ns, false);
      }
    }
  }

  fn record_method_call(&mut self, method_name: &str, current_ns: &str) {
    if !self.called_methods.insert(method_name.to_owned()) {
      return;
    }
    if self.wildcard_methods {
      return;
    }
    let impls: Vec<_> = self.core_builtin_impls.iter().chain(self.seen_impls.iter()).cloned().collect();
    for imp in impls {
      if let Some(entry) = imp.get(method_name).cloned() {
        self.extra_exprs.push(entry.clone());
        self.visit_expr(&entry, current_ns, false);
      }
    }
  }

  fn enable_wildcard_methods(&mut self, current_ns: &str) {
    if self.wildcard_methods {
      return;
    }
    self.wildcard_methods = true;
    let impls: Vec<_> = self.core_builtin_impls.iter().chain(self.seen_impls.iter()).cloned().collect();
    for imp in impls {
      for entry in imp.values.iter() {
        self.extra_exprs.push(entry.clone());
        self.visit_expr(entry, current_ns, false);
      }
    }
  }

  fn visit_struct_def(&mut self, struct_def: &CalcitStructDef, current_ns: &str) {
    if let Some(reference) = &struct_def.definition_ref {
      self.enqueue_qualified(reference.as_ref());
    }
    for imp in &struct_def.impls {
      self.record_impl(imp, current_ns);
    }
    for field_ty in struct_def.field_types.iter() {
      self.visit_annotation(field_ty, current_ns);
    }
    for bound in struct_def.where_bounds.iter() {
      for trait_def in bound.traits.iter() {
        if let Some(reference) = &trait_def.definition_ref {
          self.enqueue_qualified(reference.as_ref());
        }
      }
    }
    for default_expr in struct_def.field_defaults.iter().flatten() {
      self.extra_exprs.push(default_expr.clone());
      self.visit_expr(default_expr, current_ns, false);
    }
  }

  fn visit_enum_def(&mut self, enum_def: &CalcitEnumDef, current_ns: &str) {
    if let Some(reference) = enum_def.definition_ref() {
      self.enqueue_qualified(reference.as_ref());
    }
    for imp in &enum_def.impls {
      self.record_impl(imp, current_ns);
    }
    for variant in enum_def.variants() {
      for payload in variant.payload_types() {
        self.visit_annotation(payload, current_ns);
      }
    }
    for bound in enum_def.where_bounds() {
      for trait_def in bound.traits.iter() {
        if let Some(reference) = &trait_def.definition_ref {
          self.enqueue_qualified(reference.as_ref());
        }
      }
    }
  }

  fn visit_annotation(&mut self, annotation: &Arc<CalcitTypeAnnotation>, current_ns: &str) {
    let key = (Arc::as_ptr(annotation) as usize, current_ns.to_owned());
    if !self.visited_annotations.insert(key) {
      return;
    }
    // Keep every visited Arc alive for the lifetime of ReachabilityState so
    // temporary annotations cannot be freed and reused at the same address.
    self.retained_annotations.push(annotation.clone());
    match annotation.as_ref() {
      CalcitTypeAnnotation::List(inner)
      | CalcitTypeAnnotation::Set(inner)
      | CalcitTypeAnnotation::Ref(inner)
      | CalcitTypeAnnotation::Variadic(inner)
      | CalcitTypeAnnotation::Nullable(inner, _) => {
        self.visit_annotation(inner, current_ns);
      }
      CalcitTypeAnnotation::Tuple(slots) => {
        for slot in slots.iter() {
          self.visit_annotation(slot, current_ns);
        }
      }
      CalcitTypeAnnotation::Map(k, v) => {
        self.visit_annotation(k, current_ns);
        self.visit_annotation(v, current_ns);
      }
      CalcitTypeAnnotation::Fn(fn_sig) => {
        for arg in &fn_sig.arg_types {
          self.visit_annotation(arg, current_ns);
        }
        if let Some(rest) = &fn_sig.rest_type {
          self.visit_annotation(rest, current_ns);
        }
        self.visit_annotation(&fn_sig.return_type, current_ns);
        for bound in fn_sig.where_bounds.iter() {
          for trait_def in bound.traits.iter() {
            if let Some(reference) = &trait_def.definition_ref {
              self.enqueue_qualified(reference.as_ref());
            }
          }
        }
      }
      CalcitTypeAnnotation::Struct(def, args) => {
        self.visit_struct_def(def, current_ns);
        for arg in args.iter() {
          self.visit_annotation(arg, current_ns);
        }
      }
      CalcitTypeAnnotation::StructValue(def) | CalcitTypeAnnotation::StructDef(def) => {
        self.visit_struct_def(def, current_ns);
      }
      CalcitTypeAnnotation::Enum(def, args) => {
        self.visit_enum_def(def, current_ns);
        for arg in args.iter() {
          self.visit_annotation(arg, current_ns);
        }
      }
      CalcitTypeAnnotation::EnumValue(def) | CalcitTypeAnnotation::EnumDef(def) => {
        self.visit_enum_def(def, current_ns);
      }
      CalcitTypeAnnotation::TypeRef(name, args) => {
        self.enqueue_type_ref(current_ns, name.as_ref());
        for arg in args.iter() {
          self.visit_annotation(arg, current_ns);
        }
        let key_args: Vec<CalcitTypeAnnotation> = args.iter().map(|arg| arg.as_ref().clone()).collect();
        if self
          .active_alias_expansions
          .iter()
          .any(|(active_name, active_args)| active_name == name && active_args == &key_args)
        {
          return;
        }
        let expanded = CalcitTypeAnnotation::expand_schema_alias(annotation);
        if !Arc::ptr_eq(&expanded, annotation) {
          self.active_alias_expansions.push((name.clone(), key_args));
          self.visit_annotation(&expanded, current_ns);
          self.active_alias_expansions.pop();
        }
      }
      CalcitTypeAnnotation::Custom(val) => {
        self.visit_expr(val, current_ns, false);
      }
      _ => {}
    }
  }

  fn visit_data_shape(&mut self, graph: &DataShapeGraph, current_ns: &str) {
    for node in &graph.nodes {
      match node {
        DataShapeNode::Struct {
          nominal,
          nominal_path,
          type_args,
          ..
        } => {
          self.visit_struct_def(nominal, current_ns);
          if let Some((ns, def)) = nominal_path {
            self.enqueue(ns.as_ref(), def.as_ref());
          }
          for arg in type_args.iter() {
            self.visit_annotation(arg, current_ns);
          }
        }
        DataShapeNode::Enum {
          nominal,
          nominal_path,
          type_args,
          ..
        } => {
          self.visit_enum_def(nominal, current_ns);
          if let Some((ns, def)) = nominal_path {
            self.enqueue(ns.as_ref(), def.as_ref());
          }
          for arg in type_args.iter() {
            self.visit_annotation(arg, current_ns);
          }
        }
        DataShapeNode::MapOption { nominal, nominal_path, .. } => {
          self.visit_enum_def(nominal, current_ns);
          if let Some((ns, def)) = nominal_path {
            self.enqueue(ns.as_ref(), def.as_ref());
          }
        }
        DataShapeNode::Union {
          nominal_path: Some((ns, def)),
          ..
        } => {
          self.enqueue(ns.as_ref(), def.as_ref());
        }
        _ => {}
      }
    }
  }

  fn visit_expr(&mut self, expr: &Calcit, current_ns: &str, in_quote: bool) {
    if let Some(graph) = DataShapeGraph::from_calcit_handle(expr) {
      self.visit_data_shape(&graph, current_ns);
    }
    match expr {
      Calcit::Import(import) => {
        self.enqueue(import.ns.as_ref(), import.def.as_ref());
      }
      Calcit::Symbol { sym, info, .. } if !in_quote => {
        if let Some((sym_ns, def_name)) = sym.rsplit_once('/') {
          self.enqueue(sym_ns, def_name);
        } else {
          let target_ns = if info.at_ns.is_empty() { current_ns } else { info.at_ns.as_ref() };
          self.enqueue(target_ns, sym.as_ref());
        }
      }
      Calcit::Local(local) => {
        self.visit_annotation(&local.type_info, current_ns);
      }
      Calcit::Fn { info, .. } => {
        if let Some(def_ref) = &info.def_ref {
          self.enqueue(def_ref.def_ns.as_ref(), def_ref.def_name.as_ref());
        }
        for item in &info.body {
          self.visit_expr(item, current_ns, false);
        }
      }
      Calcit::Method(name, kind) => {
        self.record_method_call(name.as_ref(), current_ns);
        match kind {
          MethodKind::Invoke(ty) | MethodKind::ExternalAccess(ty) | MethodKind::ExternalGet(ty) | MethodKind::ExternalSet(ty) => {
            self.visit_annotation(ty, current_ns)
          }
          _ => {}
        }
      }
      Calcit::StructDef(s) => self.visit_struct_def(s, current_ns),
      Calcit::EnumDef(e) => self.visit_enum_def(e, current_ns),
      Calcit::Struct(s) => self.visit_struct_def(&s.struct_ref, current_ns),
      Calcit::Enum(e) => {
        if let Some(sum_type) = &e.sum_type {
          self.visit_enum_def(sum_type, current_ns);
        }
        for item in e.extra.iter() {
          self.visit_expr(item, current_ns, in_quote);
        }
      }
      Calcit::UnionDef(u) => {
        if let Some(reference) = u.definition_ref() {
          self.enqueue_qualified(reference.as_ref());
        }
        for member in u.members() {
          self.visit_annotation(&member, current_ns);
        }
      }
      Calcit::Impl(imp) => {
        self.record_impl(imp, current_ns);
      }
      Calcit::Thunk(thunk) => {
        self.visit_expr(thunk.get_code(), current_ns, false);
      }
      Calcit::List(xs) => {
        let is_quote = matches!(xs.first(), Some(Calcit::Syntax(CalcitSyntax::Quote | CalcitSyntax::Quasiquote, _)));
        let is_trait_call = matches!(xs.first(), Some(Calcit::Proc(CalcitProc::NativeTraitCall)))
          || matches!(
            xs.first(),
            Some(Calcit::Import(CalcitImport { ns, def, .. }))
              if ns.as_ref() == crate::wasm::host::calcit::CORE_NS && def.as_ref() == "&trait-call"
          );
        if is_trait_call {
          match xs.get(2) {
            Some(Calcit::Tag(method_tag)) => self.record_method_call(method_tag.ref_str(), current_ns),
            _ => self.enable_wildcard_methods(current_ns),
          }
        }
        for item in xs.iter() {
          self.visit_expr(item, current_ns, in_quote || is_quote);
        }
      }
      Calcit::Map(m) => {
        for (k, v) in m.iter() {
          self.visit_expr(k, current_ns, in_quote);
          self.visit_expr(v, current_ns, in_quote);
        }
      }
      Calcit::Set(s) => {
        for item in s.iter() {
          self.visit_expr(item, current_ns, in_quote);
        }
      }
      Calcit::Recur(args) => {
        for item in args {
          self.visit_expr(item, current_ns, in_quote);
        }
      }
      _ => {}
    }
  }

  fn run(&mut self) {
    while let Some((ns, name)) = self.pending.pop() {
      let key = (ns.clone(), name.clone());
      if !self.visited.insert(key) {
        continue;
      }
      let Some(compiled) = self.program_data.get(ns.as_str()).and_then(|file| file.defs.get(name.as_str())) else {
        continue;
      };
      self.visit_annotation(&compiled.schema, &ns);
      let runtime_val = program::lookup_runtime_ready(&ns, &name);
      match runtime_val.as_ref() {
        Some(Calcit::StructDef(s)) => {
          self.visit_struct_def(s, &ns);
          continue;
        }
        Some(Calcit::EnumDef(e)) => {
          self.visit_enum_def(e, &ns);
          continue;
        }
        Some(Calcit::UnionDef(union_def)) => {
          for member in union_def.members() {
            self.visit_annotation(&member, &ns);
          }
          continue;
        }
        Some(Calcit::Impl(imp)) => {
          self.record_impl(imp, &ns);
          continue;
        }
        _ => {}
      }

      if let Some(Calcit::List(items)) = &compiled.source_code
        && let Some(type_def) = crate::wasm::host::calcit::type_annotation::resolve_type_def_from_code(&Calcit::List(items.clone()))
      {
        self.visit_expr(&type_def, &ns, false);
      }

      let mut dep_names: Vec<(String, String)> = compiled
        .deps
        .iter()
        .filter_map(|dep_id| self.def_id_to_name.get(dep_id).cloned())
        .collect();
      dep_names.sort_unstable();
      for (dep_ns, dep_name) in dep_names {
        self.enqueue(&dep_ns, &dep_name);
      }
      if let Some(runtime_val) = runtime_val {
        self.visit_expr(&runtime_val, &ns, false);
      }
      self.visit_expr(&compiled.preprocessed_code, &ns, false);
      self.visit_expr(&compiled.codegen_form, &ns, false);
      if let Some(type_def) = try_parse_defrecord_form(&compiled.preprocessed_code) {
        self.visit_struct_def(&type_def, &ns);
      }
    }
  }
}

fn collect_impls_into(value: &Calcit, out: &mut Vec<Arc<crate::wasm::host::calcit::CalcitImpl>>) {
  match value {
    Calcit::Impl(imp) => out.push(imp.clone()),
    Calcit::List(items) => {
      for item in items.iter() {
        collect_impls_into(item, out);
      }
    }
    _ => {}
  }
}

pub(super) fn collect_reachable_program_defs(
  program_data: &program::CompiledProgram,
  config: ReachabilityConfig<'_>,
) -> ReachableProgram {
  let mut state = ReachabilityState::new(program_data);
  state.enqueue(config.init_ns, config.init_def);

  if config.target == WasmTarget::Core
    && let Some(file_info) = program_data.get(config.init_ns)
  {
    for (def_name, _) in super::deterministic_definition_order(file_info) {
      state.enqueue(config.init_ns, def_name.as_ref());
    }
  }

  for ns in super::deterministic_namespace_order(program_data, Some(config.init_ns)) {
    let Some(file_info) = program_data.get(ns.as_str()) else {
      continue;
    };
    for (def_name, compiled) in super::deterministic_definition_order(file_info) {
      if is_wasm_export_def(&compiled.preprocessed_code) || is_wasm_import_def(&compiled.preprocessed_code) {
        state.enqueue(ns.as_str(), def_name.as_ref());
      }
    }
  }

  for adapter in config.extra_imports {
    state.enqueue_qualified(&adapter.definition);
  }
  if config.wasi_command_component {
    state.enqueue(crate::wasm::host::calcit::CORE_NS, "get-args");
  }
  if config.wasi_command_read_dir {
    state.enqueue(crate::wasm::host::calcit::CORE_NS, "FsPath");
  }
  if config.wasi_http_proxy {
    for core_def in [
      "HttpHeader",
      "HttpRequest",
      "HttpResponse",
      "HttpMethod",
      "HttpBody",
      "Result",
      "Option",
    ] {
      state.enqueue(crate::wasm::host::calcit::CORE_NS, core_def);
    }
  }

  state.run();
  ReachableProgram {
    defs: state.visited,
    extra_exprs: state.extra_exprs,
  }
}

// ---------------------------------------------------------------------------
// Tag collection
// ---------------------------------------------------------------------------

/// Collect all Tag values from reachable function bodies and definitions and
/// build the tag → id map (1-based; 0 is reserved/unused).
pub(super) fn collect_all_tags_from(
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  program_data: Option<&program::CompiledProgram>,
  reachable_defs: Option<&HashSet<(String, String)>>,
  extra_exprs: &[Calcit],
) -> HashMap<String, u32> {
  let mut tags: Vec<String> = Vec::new();
  // Always include builtin type tags — used by `type-of` and heap headers.
  for t in BUILTIN_TYPE_TAGS {
    tags.push((*t).to_string());
  }
  for (_, _, _, body) in fn_defs {
    for expr in body {
      collect_tags_from_expr(expr, &mut tags);
    }
  }
  for expr in extra_exprs {
    collect_tags_from_expr(expr, &mut tags);
  }
  if let Some(program_data) = program_data {
    for ns in super::deterministic_namespace_order(program_data, None) {
      let Some(file) = program_data.get(ns.as_str()) else {
        continue;
      };
      for (name, compiled) in super::deterministic_definition_order(file) {
        if let Some(reachable) = reachable_defs
          && !reachable.contains(&(ns.clone(), name.to_string()))
        {
          continue;
        }
        if let Some(value @ (Calcit::StructDef(_) | Calcit::EnumDef(_))) = program::lookup_runtime_ready(&ns, name) {
          collect_tags_from_expr(&value, &mut tags);
        }
        for code in compiled
          .source_code
          .iter()
          .chain([&compiled.preprocessed_code, &compiled.codegen_form])
        {
          // Match collect_struct_layouts, including references nested in
          // reachable compiled definitions outside selected function bodies.
          collect_tags_from_expr(code, &mut tags);
          let struct_def = match code {
            Calcit::StructDef(struct_def) => Some(struct_def.as_ref().clone()),
            _ => {
              try_parse_defrecord_form(code).or_else(|| {
                match crate::wasm::host::calcit::type_annotation::resolve_type_def_from_code(code) {
                  Some(Calcit::StructDef(struct_def)) => Some(struct_def.as_ref().clone()),
                  _ => None,
                }
              })
            }
          };
          if let Some(struct_def) = struct_def {
            tags.push(struct_def.name.ref_str().to_owned());
            tags.extend(struct_def.fields.iter().map(|field| field.ref_str().to_owned()));
          }
          let enum_def = match code {
            Calcit::EnumDef(enum_def) => Some(enum_def.clone()),
            _ => match crate::wasm::host::calcit::type_annotation::resolve_type_def_from_code(code) {
              Some(Calcit::EnumDef(enum_def)) => Some(enum_def),
              _ => None,
            },
          };
          if let Some(enum_def) = enum_def {
            tags.push(enum_identity_tag(&enum_def));
            tags.extend(enum_def.variants().iter().map(|variant| variant.tag.ref_str().to_owned()));
          }
        }
      }
    }
  }
  tags.sort();
  tags.dedup();
  tags.into_iter().enumerate().map(|(i, t)| (t, (i + 1) as u32)).collect()
}

pub(super) fn collect_tags_from_expr(expr: &Calcit, tags: &mut Vec<String>) {
  if let Some(graph) = DataShapeGraph::from_calcit_handle(expr) {
    for node in &graph.nodes {
      match node {
        DataShapeNode::Struct { nominal, fields, .. } => {
          tags.push(nominal.name.ref_str().to_owned());
          tags.extend(fields.iter().map(|(field, _)| field.ref_str().to_owned()));
        }
        DataShapeNode::Enum { variants, .. } => {
          tags.extend(variants.iter().map(|(variant, _)| variant.ref_str().to_owned()));
        }
        _ => {}
      }
    }
  }
  match expr {
    Calcit::Tag(t) => {
      tags.push(t.to_string());
    }
    Calcit::Str(s) => {
      collect_json_keys_from_str(s, tags);
    }
    Calcit::Local(local) => {
      if let Some(enum_def) = expand_wasm_value_type(&local.type_info).resolve_to_enum() {
        tags.push(enum_identity_tag(&enum_def));
      }
    }
    Calcit::List(xs) => {
      for x in xs.iter() {
        collect_tags_from_expr(x, tags);
      }
    }
    Calcit::Fn { info, .. } => {
      for x in &info.body {
        collect_tags_from_expr(x, tags);
      }
    }
    Calcit::StructDef(s) => {
      tags.push(s.name.to_string());
      for f in s.fields.iter() {
        tags.push(f.to_string());
      }
    }
    Calcit::EnumDef(e) => {
      tags.push(enum_identity_tag(e));
      for v in e.variants() {
        tags.push(v.tag.to_string());
      }
    }
    Calcit::Proc(CalcitProc::NativeNumberFits) => {
      for refinement in [
        CalcitNumericRefinement::Int8,
        CalcitNumericRefinement::UInt8,
        CalcitNumericRefinement::Int16,
        CalcitNumericRefinement::UInt16,
        CalcitNumericRefinement::Int32,
        CalcitNumericRefinement::UInt32,
        CalcitNumericRefinement::Int64,
        CalcitNumericRefinement::UInt64,
        CalcitNumericRefinement::Float32,
        CalcitNumericRefinement::Float64,
      ] {
        tags.push(refinement.tag_name().to_owned());
      }
    }
    Calcit::Method(name, _) if name.as_ref() == "filter-map-kv" => {
      tags.push("keep".to_owned());
      tags.push("drop".to_owned());
    }
    Calcit::Symbol { sym, .. } if sym.as_ref() == "filter-map-kv" => {
      tags.push("keep".to_owned());
      tags.push("drop".to_owned());
    }
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      if def.as_ref() == "filter-map-kv" {
        tags.push("keep".to_owned());
        tags.push("drop".to_owned());
      }
      if let Ok(struct_def) = resolve_struct_ref(expr) {
        tags.push(struct_def.name.to_string());
        for f in struct_def.fields.iter() {
          tags.push(f.to_string());
        }
      } else if let Ok(enum_def) = resolve_enum_ref(expr) {
        tags.push(enum_identity_tag(&enum_def));
        for v in enum_def.variants() {
          tags.push(v.tag.to_string());
        }
      }
      let _ = ns;
    }
    _ => {}
  }
}

// ---------------------------------------------------------------------------
// Static string & empty-container data pool
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(super) fn build_string_pool(
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  value_imports: &HashMap<String, Calcit>,
  tag_index: &HashMap<String, u32>,
  target: WasmTarget,
) -> (HashMap<String, u32>, Vec<u8>, i32) {
  build_string_pool_with_extras(
    fn_defs,
    value_imports,
    &[],
    tag_index,
    StaticPoolOptions {
      target,
      wasi_stdio: target == WasmTarget::Wasi && fn_defs.iter().any(|(_, _, _, body)| body.iter().any(super::expr_uses_wasi_stdio)),
      wasi_http_proxy: false,
      wasi_read_dir: target == WasmTarget::Wasi,
      needs_json_parse: true,
      needs_value_formatting: true,
    },
  )
}

pub(super) fn detect_static_pool_options(
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  value_imports: &HashMap<String, Calcit>,
  extra_exprs: &[Calcit],
  target: WasmTarget,
  wasi_http_proxy: bool,
  wasi_stdio: bool,
  wasi_read_dir: bool,
) -> StaticPoolOptions {
  let any_expr = |pred: fn(&Calcit) -> bool| -> bool {
    fn_defs.iter().any(|(_, _, _, body)| body.iter().any(pred)) || value_imports.values().any(pred) || extra_exprs.iter().any(pred)
  };
  let needs_json_parse = fn_defs
    .iter()
    .any(|(ns, name, _, _)| ns == crate::wasm::host::calcit::CORE_NS && name == "try-parse-json")
    || any_expr(expr_uses_json_parse);
  let needs_value_formatting = fn_defs
    .iter()
    .any(|(ns, name, _, _)| ns == crate::wasm::host::calcit::CORE_NS && matches!(name.as_str(), "join-string" | "to-string"))
    || any_expr(expr_uses_value_formatting);
  StaticPoolOptions {
    target,
    wasi_stdio: wasi_stdio || (target == WasmTarget::Wasi && any_expr(super::expr_uses_wasi_stdio)),
    wasi_http_proxy,
    wasi_read_dir: wasi_read_dir || (target == WasmTarget::Wasi && any_expr(super::expr_uses_wasi_read_dir)),
    needs_json_parse,
    needs_value_formatting,
  }
}

pub(super) fn build_string_pool_with_extras(
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  value_imports: &HashMap<String, Calcit>,
  extra_exprs: &[Calcit],
  tag_index: &HashMap<String, u32>,
  options: StaticPoolOptions,
) -> (HashMap<String, u32>, Vec<u8>, i32) {
  let mut strings: Vec<String> = Vec::new();
  let mut needs_edn_format = false;
  let mut needs_edn_parse = false;
  let mut needs_json_format = false;
  for (_, _, _, body) in fn_defs {
    for expr in body {
      collect_strings_from_expr(expr, &mut strings, options.needs_value_formatting);
      needs_edn_format |= expr_uses_cirru_edn_format(expr);
      needs_edn_parse |= expr_uses_typed_cirru_edn_parse(expr);
      needs_json_format |= expr_uses_json_format(expr);
    }
  }
  for expr in value_imports.values().chain(extra_exprs.iter()) {
    collect_strings_from_expr(expr, &mut strings, options.needs_value_formatting);
    needs_edn_format |= expr_uses_cirru_edn_format(expr);
    needs_edn_parse |= expr_uses_typed_cirru_edn_parse(expr);
    needs_json_format |= expr_uses_json_format(expr);
  }
  if options.target == WasmTarget::Wasi && options.wasi_stdio {
    strings.push(" ".into());
    strings.push("\n".into());
  }
  if options.target == WasmTarget::Wasi && options.wasi_read_dir {
    strings.push("E_WASM_CAPABILITY: recursive directory reads are not supported on WASI".into());
  }
  if options.wasi_http_proxy {
    strings.push(http_proxy::HTTP_STATIC_CONTENT_TYPE_KEY.into());
    strings.push(http_proxy::HTTP_STATIC_CONTENT_TYPE_TEXT_PLAIN.into());
    strings.push(http_proxy::HTTP_STATIC_BAD_REQUEST_MSG.into());
    strings.push(http_proxy::HTTP_STATIC_BODY_TOO_LARGE_MSG.into());
    strings.push(http_proxy::HTTP_STATIC_INTERNAL_ERROR_MSG.into());
  }
  if options.needs_json_parse {
    strings.push("E_WASM_EDN_SYNTAX: invalid JSON input".into());
  }
  if options.needs_value_formatting || needs_edn_format || needs_edn_parse || needs_json_format {
    strings.push("true".into());
    strings.push("false".into());
    strings.extend(tag_index.keys().cloned());
  }
  if options.needs_value_formatting || needs_edn_format || needs_edn_parse {
    strings.extend(tag_index.keys().map(|tag| format!(":{tag}")));
  }
  if needs_edn_format || needs_edn_parse {
    strings.extend(tag_index.keys().map(|tag| format!("'{tag}")));
  }
  if needs_json_format {
    strings.extend(
      [
        "null", "true", "false", "[]", "{}", "[", "]", "{", "}", ",", ":", ": ", ",\n", "[\n", "{\n", "\n",
      ]
      .into_iter()
      .map(String::from),
    );
    for depth in 0..=33 {
      let indent = "  ".repeat(depth);
      strings.push(format!("\n{indent}"));
      strings.push(indent);
    }
  }
  if needs_edn_format {
    strings.extend(
      [
        "do ", "\n", "nil", "true", "false", "[]", "([]", "{}", "({}", "%{}", "(%{}", "%::", "(%::", ")", " ", " (",
      ]
      .into_iter()
      .map(String::from),
    );
  }
  if needs_edn_parse {
    strings.extend(
      [
        "",
        "[",
        "]",
        ".",
        ".item",
        ".key",
        ".value",
        ".payload[",
        " at $",
        "do ",
        "nil",
        "true",
        "false",
        "E_WASM_EDN_INPUT_LIMIT: Cirru EDN input exceeds 65536 bytes",
        "E_WASM_EDN_SYNTAX: invalid Cirru EDN scalar",
        "E_WASM_EDN_RANGE: numeric value is outside the requested type",
        "E_WASM_EDN_TAG: tag is not present in the compiled program",
        "E_WASM_EDN_TOKEN_LIMIT: Cirru EDN list exceeds 4096 items",
        "E_WASM_EDN_MAP_LIMIT: Cirru EDN map exceeds 2048 entries",
        "E_WASM_EDN_ENUM: Cirru EDN enum type, variant, or payload does not match the requested type",
      ]
      .into_iter()
      .map(String::from),
    );
  }
  strings.sort();
  strings.dedup();

  if strings.is_empty() {
    return (HashMap::new(), Vec::new(), HEAP_BASE);
  }

  let string_tag_id = *tag_index.get("string").expect("string type tag must exist") as i32;
  let mut pool: HashMap<String, u32> = HashMap::new();
  let mut data: Vec<u8> = Vec::new();
  let mut offset = HEAP_BASE as u32;

  for s in &strings {
    let byte_len = s.len() as u32;
    data.extend_from_slice(&(HEAP_MAGIC as u32).to_le_bytes());
    data.extend_from_slice(&(string_tag_id as u32).to_le_bytes());
    let logical_ptr = offset + 8;
    pool.insert(s.clone(), logical_ptr);
    data.extend_from_slice(&(byte_len as f64).to_le_bytes());
    data.extend_from_slice(s.as_bytes());
    let padded_len = (byte_len + 7) & !7;
    data.extend(std::iter::repeat_n(0u8, (padded_len - byte_len) as usize));
    offset += 8 + 8 + padded_len;
  }

  let heap_start = offset as i32;
  (pool, data, heap_start)
}

pub(super) struct StaticDataAndAtoms {
  pub(super) string_pool: HashMap<String, u32>,
  pub(super) string_data_segment: Vec<u8>,
  pub(super) heap_start: i32,
  pub(super) atom_initial_values: Vec<f64>,
  pub(super) atom_globals: HashMap<String, u32>,
}

fn append_static_empty_container(
  data_segment: &mut Vec<u8>,
  heap_start: &mut i32,
  tag_index: &HashMap<String, u32>,
  tag_name: &str,
  f64_slots: usize,
) -> f64 {
  let tag_id = *tag_index.get(tag_name).unwrap_or_else(|| panic!("{tag_name} type tag must exist"));
  let logical_ptr = (*heap_start as u32) + 8;
  data_segment.extend_from_slice(&(HEAP_MAGIC as u32).to_le_bytes());
  data_segment.extend_from_slice(&tag_id.to_le_bytes());
  for _ in 0..f64_slots {
    data_segment.extend_from_slice(&0.0f64.to_le_bytes());
  }
  *heap_start += 8 + (f64_slots as i32) * 8;
  logical_ptr as f64
}

pub(super) fn build_static_data_and_atoms(
  fn_defs: &[(String, String, CalcitFnArgs, Vec<Calcit>)],
  value_imports: &HashMap<String, Calcit>,
  tag_index: &HashMap<String, u32>,
  pool_options: StaticPoolOptions,
  defref_defs: &[(String, Option<Calcit>)],
  extra_reachable_exprs: &[Calcit],
) -> StaticDataAndAtoms {
  let mut extra_exprs: Vec<Calcit> = defref_defs.iter().filter_map(|(_, init)| init.clone()).collect();
  extra_exprs.extend(extra_reachable_exprs.iter().cloned());
  let (string_pool, mut data_segment, mut heap_start) =
    build_string_pool_with_extras(fn_defs, value_imports, &extra_exprs, tag_index, pool_options);
  let mut atom_initial_values: Vec<f64> = Vec::with_capacity(defref_defs.len());
  let mut atom_globals: HashMap<String, u32> = HashMap::with_capacity(defref_defs.len());
  let mut empty_list_ptr: Option<f64> = None;
  let mut empty_set_ptr: Option<f64> = None;
  let mut empty_map_ptr: Option<f64> = None;

  for (qualified, init_opt) in defref_defs {
    let global_idx = 1 + atom_initial_values.len() as u32;
    atom_globals.insert(qualified.clone(), global_idx);
    let mut current = init_opt.as_ref();
    while let Some(Calcit::List(items)) = current
      && matches!(items.first(), Some(Calcit::Syntax(CalcitSyntax::AssertType, _)))
      && items.len() >= 2
    {
      current = Some(&items[1]);
    }
    let static_empty = match current {
      Some(Calcit::Proc(CalcitProc::List)) => Some(("list", 1usize, &mut empty_list_ptr)),
      Some(Calcit::Import(imp)) if imp.def.as_ref() == "[]" => Some(("list", 1, &mut empty_list_ptr)),
      Some(Calcit::List(xs)) if xs.len() == 1 && matches!(xs.first(), Some(Calcit::Proc(CalcitProc::List))) => {
        Some(("list", 1, &mut empty_list_ptr))
      }
      Some(Calcit::Proc(CalcitProc::Set)) => Some(("set", 1, &mut empty_set_ptr)),
      Some(Calcit::Import(imp)) if imp.def.as_ref() == "#{}" => Some(("set", 1, &mut empty_set_ptr)),
      Some(Calcit::List(xs)) if xs.len() == 1 && matches!(xs.first(), Some(Calcit::Proc(CalcitProc::Set))) => {
        Some(("set", 1, &mut empty_set_ptr))
      }
      Some(Calcit::Proc(CalcitProc::NativeMap)) => Some(("map", 2, &mut empty_map_ptr)),
      Some(Calcit::Import(imp)) if imp.def.as_ref() == "{}" => Some(("map", 2, &mut empty_map_ptr)),
      Some(Calcit::List(xs)) if xs.len() == 1 && matches!(xs.first(), Some(Calcit::Proc(CalcitProc::NativeMap))) => {
        Some(("map", 2, &mut empty_map_ptr))
      }
      _ => None,
    };
    let init_val = if let Some((tag_name, f64_slots, cache)) = static_empty {
      *cache.get_or_insert_with(|| append_static_empty_container(&mut data_segment, &mut heap_start, tag_index, tag_name, f64_slots))
    } else {
      match current {
        None | Some(Calcit::Nil) | Some(Calcit::Bool(false)) => 0.0,
        Some(Calcit::Bool(true)) => 1.0,
        Some(Calcit::Number(n)) => *n,
        Some(Calcit::Tag(tag)) => tag_index.get(tag.ref_str()).copied().map_or(0.0, f64::from),
        Some(Calcit::Str(s)) => string_pool.get(s.as_ref()).copied().map_or(0.0, f64::from),
        _ => 0.0,
      }
    };
    atom_initial_values.push(init_val);
  }
  debug_assert_eq!(heap_start % 8, 0);
  StaticDataAndAtoms {
    string_pool,
    string_data_segment: data_segment,
    heap_start,
    atom_initial_values,
    atom_globals,
  }
}

/// If `expr` is a literal enum constructor `(NativeEnum :tag val0 val1...)` with only
/// literal args (Tag, Str, Number, Bool, Nil, Unit), return its lispy string representation.
/// Used both to pre-intern the string and to emit it as a constant in `emit_turn_string`.
pub(crate) fn try_format_enum_literal(expr: &Calcit) -> Option<String> {
  if let Calcit::List(list) = expr
    && !list.is_empty()
    && let Calcit::Proc(p) = &list[0]
    && *p == CalcitProc::NativeEnum
  {
    let mut s = String::from("(%:: _ ");
    for (i, item) in list.iter().skip(1).enumerate() {
      if i > 0 {
        s.push(' ');
      }
      match item {
        Calcit::Tag(_) | Calcit::Str(_) | Calcit::Number(_) | Calcit::Bool(_) | Calcit::Nil | Calcit::Unit => {
          use std::fmt::Write;
          write!(s, "{item}").ok()?;
        }
        _ => return None,
      }
    }
    s.push(')');
    return Some(s);
  }
  None
}

fn collect_strings_from_data_shape(graph: &DataShapeGraph, strings: &mut Vec<String>) {
  for node in &graph.nodes {
    match node {
      DataShapeNode::Struct { nominal, fields, .. } => {
        strings.push(format!("'{}", nominal.name));
        strings.push(format!(":{}", nominal.name));
        for (field, _) in fields {
          strings.push(format!(":{field}"));
        }
      }
      DataShapeNode::Enum { nominal, variants, .. } => {
        strings.push(format!("'{}", nominal.name()));
        strings.push(format!(":{}", nominal.name()));
        for (variant, _) in variants {
          strings.push(format!("'{variant}"));
          strings.push(format!(":{variant}"));
        }
      }
      _ => {}
    }
  }
}

fn collect_strings_from_expr(expr: &Calcit, strings: &mut Vec<String>, include_number_literals: bool) {
  if let Some(graph) = DataShapeGraph::from_calcit_handle(expr) {
    collect_strings_from_data_shape(&graph, strings);
  }
  match expr {
    Calcit::Number(number) if include_number_literals => {
      strings.push(crate::wasm::host::util::number::format_calcit_number(*number));
    }
    Calcit::Str(s) => {
      strings.push(s.to_string());
    }
    // Intern tag names as strings so emit_ptr_to_i32 can pass literal tags to string procs.
    Calcit::Tag(t) => {
      strings.push(t.to_string());
    }
    Calcit::List(xs) => {
      if xs.len() == 1 && super::expr_uses_wasi_stdio(expr) {
        strings.push(String::new());
      }
      if xs.len() == 2
        && let Calcit::Proc(p) = &xs[0]
        && matches!(p.as_ref(), "format-to-lisp")
        && let Calcit::List(inner) = &xs[1]
        && inner.len() >= 2
        && let Calcit::Syntax(CalcitSyntax::Quote, _) = &inner[0]
      {
        let s = crate::wasm::host::calcit::format_to_lisp(&inner[1]);
        strings.push(s);
      }
      if include_number_literals && let Some(enum_str) = try_format_enum_literal(expr) {
        strings.push(enum_str);
      }
      if matches!(xs.first(), Some(Calcit::Proc(CalcitProc::FormatCirruEdn)))
        && let Some(value) = xs.get(1)
      {
        if let Some(formatted) = edn::try_format_cirru_edn_literal(value) {
          strings.push(formatted);
        }
        if let Some(value_type) = infer_static_type_from_expr(value)
          && let Ok(graph) = DataShapeGraph::build(value_type.as_ref(), "")
        {
          collect_strings_from_data_shape(&graph, strings);
        }
      }
      for x in xs.iter() {
        collect_strings_from_expr(x, strings, include_number_literals);
      }
    }
    Calcit::Fn { info, .. } => {
      for x in &info.body {
        collect_strings_from_expr(x, strings, include_number_literals);
      }
    }
    _ => {}
  }
}

fn expr_uses_cirru_edn_format(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(xs) => {
      matches!(xs.first(), Some(Calcit::Proc(CalcitProc::FormatCirruEdn))) || xs.iter().any(expr_uses_cirru_edn_format)
    }
    Calcit::Fn { info, .. } => info.body.iter().any(expr_uses_cirru_edn_format),
    _ => false,
  }
}

fn expr_uses_json_format(expr: &Calcit) -> bool {
  match expr {
    Calcit::Proc(CalcitProc::JsonStringify | CalcitProc::JsonPretty) => true,
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      ns.as_ref() == "calcit.core" && matches!(def.as_ref(), "json-stringify" | "json-pretty")
    }
    Calcit::Symbol { sym, info, .. } => {
      info.at_ns.as_ref() == "calcit.core" && matches!(sym.as_ref(), "json-stringify" | "json-pretty")
    }
    Calcit::List(xs) => xs.iter().any(expr_uses_json_format),
    Calcit::Fn { info, .. } => info.body.iter().any(expr_uses_json_format),
    _ => false,
  }
}

fn expr_uses_json_parse(expr: &Calcit) -> bool {
  match expr {
    Calcit::Proc(CalcitProc::JsonParse | CalcitProc::NativeTryParseJson) => true,
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      ns.as_ref() == "calcit.core" && matches!(def.as_ref(), "try-parse-json" | "&try-parse-json" | "json-parse")
    }
    Calcit::Symbol { sym, .. } => matches!(sym.as_ref(), "try-parse-json" | "&try-parse-json" | "json-parse"),
    Calcit::Method(name, _) => name.as_ref() == "parse-json",
    Calcit::List(xs) => xs.iter().any(expr_uses_json_parse),
    Calcit::Fn { info, .. } => info.body.iter().any(expr_uses_json_parse),
    _ => false,
  }
}

fn expr_uses_value_formatting(expr: &Calcit) -> bool {
  const CORE_FMT_NAMES: [&str; 10] = [
    "str",
    "str-spaced",
    "join-string",
    "turn-string",
    "&str",
    "&str:concat",
    "println",
    "eprintln",
    "echo",
    "to-string",
  ];
  match expr {
    Calcit::Proc(CalcitProc::TurnString | CalcitProc::NativeStr | CalcitProc::NativeStrConcat) => true,
    Calcit::Registered(name) => matches!(name.as_ref(), "println" | "eprintln" | "echo"),
    Calcit::Import(CalcitImport { ns, def, .. }) => {
      ns.as_ref() == crate::wasm::host::calcit::CORE_NS && CORE_FMT_NAMES.contains(&def.as_ref())
    }
    Calcit::Symbol { sym, .. } => CORE_FMT_NAMES.contains(&sym.as_ref()),
    Calcit::Method(name, _) => matches!(name.as_ref(), "to-string" | "join-string" | "mappend"),
    Calcit::List(xs) => xs.iter().any(expr_uses_value_formatting),
    Calcit::Fn { info, .. } => {
      if let Some(def_ref) = &info.def_ref
        && def_ref.def_ns.as_ref() == crate::wasm::host::calcit::CORE_NS
        && CORE_FMT_NAMES.contains(&def_ref.def_name.as_ref())
      {
        return true;
      }
      info.body.iter().any(expr_uses_value_formatting)
    }
    _ => false,
  }
}

fn collect_json_keys_from_str(s: &str, tags: &mut Vec<String>) {
  if !s.contains('"') || !s.contains(':') {
    return;
  }
  let bytes = s.as_bytes();
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] != b'"' {
      i += 1;
      continue;
    }
    i += 1;
    let mut unescaped = String::new();
    let mut closed = false;
    let mut valid = true;
    while i < bytes.len() {
      let b = bytes[i];
      if b == b'"' {
        i += 1;
        closed = true;
        break;
      }
      if b == b'\\' {
        i += 1;
        if i >= bytes.len() {
          break;
        }
        match bytes[i] {
          b'"' => unescaped.push('"'),
          b'\\' => unescaped.push('\\'),
          b'/' => unescaped.push('/'),
          b'b' => unescaped.push('\u{0008}'),
          b'f' => unescaped.push('\u{000c}'),
          b'n' => unescaped.push('\n'),
          b'r' => unescaped.push('\r'),
          b't' => unescaped.push('\t'),
          b'u' => {
            if i + 4 < bytes.len() && bytes[i + 1..i + 5].iter().all(|b| b.is_ascii_hexdigit()) {
              if let Ok(code) = u32::from_str_radix(&s[i + 1..i + 5], 16) {
                if (0xD800..=0xDBFF).contains(&code)
                  && i + 10 < bytes.len()
                  && bytes[i + 5] == b'\\'
                  && bytes[i + 6] == b'u'
                  && bytes[i + 7..i + 11].iter().all(|b| b.is_ascii_hexdigit())
                  && let Ok(low) = u32::from_str_radix(&s[i + 7..i + 11], 16)
                  && (0xDC00..=0xDFFF).contains(&low)
                {
                  let scalar = 0x10000 + (((code - 0xD800) << 10) | (low - 0xDC00));
                  if let Some(ch) = char::from_u32(scalar) {
                    unescaped.push(ch);
                  } else {
                    valid = false;
                  }
                  i += 10;
                } else if let Some(ch) = char::from_u32(code) {
                  unescaped.push(ch);
                  i += 4;
                } else {
                  valid = false;
                  i += 4;
                }
              } else {
                valid = false;
                i += 4;
              }
            } else {
              valid = false;
            }
          }
          _ => {
            if let Some(ch) = s[i..].chars().next() {
              unescaped.push(ch);
              i += ch.len_utf8();
              continue;
            }
            break;
          }
        }
        i += 1;
      } else {
        let rest = &s[i..];
        if let Some(ch) = rest.chars().next() {
          unescaped.push(ch);
          i += ch.len_utf8();
        } else {
          break;
        }
      }
    }
    if !closed {
      break;
    }
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
      i += 1;
    }
    if valid && i < bytes.len() && bytes[i] == b':' {
      tags.push(unescaped);
    }
  }
}

fn expr_uses_typed_cirru_edn_parse(expr: &Calcit) -> bool {
  match expr {
    Calcit::List(xs) => {
      matches!(
        xs.first(),
        Some(Calcit::Syntax(
          CalcitSyntax::TryParseCirruEdnAs
            | CalcitSyntax::ParseCirruEdnAs
            | CalcitSyntax::DecodeMapAs
            | CalcitSyntax::TryDecodeMapAs
            | CalcitSyntax::EncodeMapAs,
          _
        ))
      ) || xs.iter().any(expr_uses_typed_cirru_edn_parse)
    }
    Calcit::Fn { info, .. } => info.body.iter().any(expr_uses_typed_cirru_edn_parse),
    _ => false,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::wasm::host::calcit::{CalcitImpl, CalcitImport, CalcitStructDef, CalcitSymbolInfo};
  use cirru_edn::EdnTag;

  fn dummy_compiled_def(def_id: u32, kind: program::CompiledDefKind, code: Calcit) -> program::CompiledDef {
    program::CompiledDef {
      def_id: program::DefId(def_id),
      version_id: 0,
      kind,
      preprocessed_code: code.clone(),
      codegen_form: code,
      deps: vec![],
      type_summary: None,
      source_code: None,
      schema: DYNAMIC_TYPE.clone(),
      proven_guard: None,
      doc: Arc::from(""),
      examples: vec![],
    }
  }

  fn fn_import(ns: &str, def: &str) -> Calcit {
    Calcit::Import(CalcitImport {
      ns: Arc::from(ns),
      def: Arc::from(def),
      info: Arc::new(crate::wasm::host::calcit::ImportInfo::SameFile {
        at_def: Arc::from("main!"),
      }),
      def_id: None,
    })
  }

  #[test]
  fn pure_arithmetic_elides_tags_and_static_strings() {
    let body = vec![Calcit::from(vec![
      Calcit::Proc(CalcitProc::NativeAdd),
      Calcit::Number(1.0),
      Calcit::Number(2.0),
    ])];
    let fn_defs = vec![("app.main".to_owned(), "main!".to_owned(), CalcitFnArgs::Args(vec![]), body)];
    let mut program_data: program::CompiledProgram = HashMap::new();
    let mut core_defs = HashMap::new();
    core_defs.insert(
      Arc::from("unused-core-tag-holder"),
      dummy_compiled_def(1, program::CompiledDefKind::Fn, Calcit::Tag(EdnTag::new("unused-core-tag"))),
    );
    program_data.insert(Arc::from("calcit.core"), program::CompiledFileData { defs: core_defs });
    let mut app_defs = HashMap::new();
    app_defs.insert(
      Arc::from("main!"),
      dummy_compiled_def(2, program::CompiledDefKind::Fn, fn_defs[0].3[0].clone()),
    );
    program_data.insert(Arc::from("app.main"), program::CompiledFileData { defs: app_defs });

    let reachable = collect_reachable_program_defs(
      &program_data,
      ReachabilityConfig {
        init_ns: "app.main",
        init_def: "main!",
        target: WasmTarget::Core,
        extra_imports: &[],
        wasi_command_component: false,
        wasi_http_proxy: false,
        wasi_command_read_dir: false,
      },
    );
    assert!(reachable.defs.contains(&(String::from("app.main"), String::from("main!"))));
    assert!(
      !reachable
        .defs
        .contains(&(String::from("calcit.core"), String::from("unused-core-tag-holder")))
    );

    let tag_index = collect_all_tags_from(&fn_defs, Some(&program_data), Some(&reachable.defs), &reachable.extra_exprs);
    assert!(!tag_index.contains_key("unused-core-tag"));

    let options = detect_static_pool_options(&fn_defs, &HashMap::new(), &[], WasmTarget::Core, false, false, false);
    assert!(!options.needs_value_formatting);
    assert!(!options.needs_json_parse);

    let (pool, data, heap_start) = build_string_pool_with_extras(&fn_defs, &HashMap::new(), &[], &tag_index, options);
    assert!(pool.is_empty(), "pure arithmetic should have an empty string pool, got {pool:?}");
    assert!(data.is_empty());
    assert_eq!(heap_start, HEAP_BASE);
  }

  #[test]
  fn formatting_edn_json_http_and_read_dir_flags_retain_required_literals() {
    let body = vec![Calcit::from(vec![
      fn_import("calcit.core", "str"),
      Calcit::Tag(EdnTag::new("status")),
      Calcit::Number(42.0),
    ])];
    let fn_defs = vec![("app.main".to_owned(), "main!".to_owned(), CalcitFnArgs::Args(vec![]), body)];
    let tag_index = collect_all_tags_from(&fn_defs, None, None, &[]);

    let options = detect_static_pool_options(&fn_defs, &HashMap::new(), &[], WasmTarget::Wasi, true, true, true);
    assert!(options.needs_value_formatting);
    assert!(options.wasi_stdio);
    assert!(options.wasi_http_proxy);
    assert!(options.wasi_read_dir);

    let (pool, data, _) = build_string_pool_with_extras(&fn_defs, &HashMap::new(), &[], &tag_index, options);
    assert!(pool.contains_key("true"));
    assert!(pool.contains_key("false"));
    assert!(pool.contains_key("status"));
    assert!(pool.contains_key(":status"));
    assert!(pool.contains_key("42"));
    assert!(pool.contains_key(" "));
    assert!(pool.contains_key("\n"));
    assert!(pool.contains_key(http_proxy::HTTP_STATIC_CONTENT_TYPE_KEY));
    assert!(pool.contains_key("E_WASM_CAPABILITY: recursive directory reads are not supported on WASI"));
    assert!(!pool.contains_key("E_WASM_EDN_SYNTAX: invalid JSON input"));
    assert!(!data.is_empty());
  }

  #[test]
  fn reachability_follows_matching_impl_methods_only() {
    let mut point_def = CalcitStructDef::from_fields(EdnTag::new("Point"), vec![EdnTag::new("x")]);
    point_def.definition_ref = Some(Arc::from("app.types/Point"));
    point_def.impls = vec![Arc::new(CalcitImpl {
      name: EdnTag::new("PointOps"),
      origin: None,
      fields: Arc::new(vec![EdnTag::new("show"), EdnTag::new("unused")]),
      values: Arc::new(vec![
        fn_import("app.helpers", "show-point"),
        fn_import("app.helpers", "dead-helper"),
      ]),
    })];

    let method_sym = Calcit::Method(Arc::from("show"), MethodKind::Invoke(DYNAMIC_TYPE.clone()));
    let main_body = Calcit::from(vec![method_sym, Calcit::StructDef(Arc::new(point_def.clone()))]);

    let mut program_data: program::CompiledProgram = HashMap::new();
    program_data.insert(
      Arc::from("app.main"),
      program::CompiledFileData {
        defs: HashMap::from([(Arc::from("main!"), dummy_compiled_def(1, program::CompiledDefKind::Fn, main_body))]),
      },
    );
    program_data.insert(
      Arc::from("app.types"),
      program::CompiledFileData {
        defs: HashMap::from([(
          Arc::from("Point"),
          dummy_compiled_def(2, program::CompiledDefKind::Value, Calcit::StructDef(Arc::new(point_def))),
        )]),
      },
    );
    program_data.insert(
      Arc::from("app.helpers"),
      program::CompiledFileData {
        defs: HashMap::from([
          (
            Arc::from("show-point"),
            dummy_compiled_def(
              3,
              program::CompiledDefKind::Fn,
              Calcit::Symbol {
                sym: Arc::from("ok"),
                info: Arc::new(CalcitSymbolInfo {
                  at_ns: Arc::from("app.helpers"),
                  at_def: Arc::from("show-point"),
                }),
                location: None,
              },
            ),
          ),
          (
            Arc::from("dead-helper"),
            dummy_compiled_def(4, program::CompiledDefKind::Fn, Calcit::Number(0.0)),
          ),
        ]),
      },
    );

    let reachable = collect_reachable_program_defs(
      &program_data,
      ReachabilityConfig {
        init_ns: "app.main",
        init_def: "main!",
        target: WasmTarget::Wasi,
        extra_imports: &[],
        wasi_command_component: false,
        wasi_http_proxy: false,
        wasi_command_read_dir: false,
      },
    );
    assert!(reachable.defs.contains(&(String::from("app.types"), String::from("Point"))));
    assert!(reachable.defs.contains(&(String::from("app.helpers"), String::from("show-point"))));
    assert!(!reachable.defs.contains(&(String::from("app.helpers"), String::from("dead-helper"))));
  }

  #[test]
  fn reached_impl_method_lambda_retains_static_strings_only_when_called() {
    use crate::wasm::host::calcit::{CalcitFn, CalcitFnCallShape, CalcitFnUsageMeta, CalcitScope};

    let make_lambda = |name: &str, msg: &str| Calcit::Fn {
      id: Arc::from(format!("fn-{name}")),
      info: Arc::new(CalcitFn {
        name: Arc::from(name),
        def_ns: Arc::from("calcit.core"),
        def_ref: None,
        usage: CalcitFnUsageMeta::default(),
        scope: Arc::new(CalcitScope::default()),
        args: Arc::new(CalcitFnArgs::Args(vec![])),
        call_shape: CalcitFnCallShape::fixed(0),
        body: vec![Calcit::from(vec![Calcit::Proc(CalcitProc::Raise), Calcit::Str(Arc::from(msg))])],
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        return_type: DYNAMIC_TYPE.clone(),
        arg_types: vec![],
        rest_type: None,
      }),
    };

    let mut box_def = CalcitStructDef::from_fields(EdnTag::new("Box"), vec![EdnTag::new("x")]);
    box_def.definition_ref = Some(Arc::from("app.types/Box"));
    box_def.impls = vec![Arc::new(CalcitImpl {
      name: EdnTag::new("BoxOps"),
      origin: None,
      fields: Arc::new(vec![EdnTag::new("contains?"), EdnTag::new("unused")]),
      values: Arc::new(vec![
        make_lambda("contains?", "Enum .contains? expected a Number index"),
        make_lambda("unused", "unused impl error message"),
      ]),
    })];

    let method_sym = Calcit::Method(Arc::from("contains?"), MethodKind::Invoke(DYNAMIC_TYPE.clone()));
    let main_body = Calcit::from(vec![method_sym, Calcit::StructDef(Arc::new(box_def.clone()))]);
    let fn_defs = vec![(
      "app.main".to_owned(),
      "main!".to_owned(),
      CalcitFnArgs::Args(vec![]),
      vec![main_body.clone()],
    )];

    let mut program_data: program::CompiledProgram = HashMap::new();
    program_data.insert(
      Arc::from("app.main"),
      program::CompiledFileData {
        defs: HashMap::from([(Arc::from("main!"), dummy_compiled_def(1, program::CompiledDefKind::Fn, main_body))]),
      },
    );
    program_data.insert(
      Arc::from("app.types"),
      program::CompiledFileData {
        defs: HashMap::from([(
          Arc::from("Box"),
          dummy_compiled_def(2, program::CompiledDefKind::Value, Calcit::StructDef(Arc::new(box_def))),
        )]),
      },
    );

    let reachable = collect_reachable_program_defs(
      &program_data,
      ReachabilityConfig {
        init_ns: "app.main",
        init_def: "main!",
        target: WasmTarget::Core,
        extra_imports: &[],
        wasi_command_component: false,
        wasi_http_proxy: false,
        wasi_command_read_dir: false,
      },
    );
    let tag_index = collect_all_tags_from(&fn_defs, Some(&program_data), Some(&reachable.defs), &reachable.extra_exprs);
    let options = detect_static_pool_options(
      &fn_defs,
      &HashMap::new(),
      &reachable.extra_exprs,
      WasmTarget::Core,
      false,
      false,
      false,
    );
    let StaticDataAndAtoms { string_pool, .. } =
      build_static_data_and_atoms(&fn_defs, &HashMap::new(), &tag_index, options, &[], &reachable.extra_exprs);

    assert!(string_pool.contains_key("Enum .contains? expected a Number index"));
    assert!(!string_pool.contains_key("unused impl error message"));
  }

  #[test]
  fn visit_annotation_retains_temporary_arcs_and_tracks_namespace() {
    let mut program_data: program::CompiledProgram = HashMap::new();
    program_data.insert(
      Arc::from("app.a"),
      program::CompiledFileData {
        defs: HashMap::from([(
          Arc::from("Helper"),
          dummy_compiled_def(10, program::CompiledDefKind::Fn, Calcit::Nil),
        )]),
      },
    );
    program_data.insert(
      Arc::from("app.b"),
      program::CompiledFileData {
        defs: HashMap::from([(
          Arc::from("Helper"),
          dummy_compiled_def(11, program::CompiledDefKind::Fn, Calcit::Nil),
        )]),
      },
    );

    let mut state = ReachabilityState::new(&program_data);
    let shared = Arc::new(CalcitTypeAnnotation::TypeRef(Arc::from("Helper"), Arc::new(vec![])));
    let weak_shared = Arc::downgrade(&shared);
    state.visit_annotation(&shared, "app.a");
    // Visiting the same Arc under another namespace must still traverse that namespace,
    // while repeating the same (pointer, namespace) pair is deduplicated.
    state.visit_annotation(&shared, "app.b");
    state.visit_annotation(&shared, "app.b");
    drop(shared);
    assert!(
      weak_shared.upgrade().is_some(),
      "ReachabilityState must retain visited temporary Arc annotations so addresses cannot be reused"
    );
    assert_eq!(state.retained_annotations.len(), 2);

    state.run();
    assert!(state.visited.contains(&(String::from("app.a"), String::from("Helper"))));
    assert!(state.visited.contains(&(String::from("app.b"), String::from("Helper"))));
  }

  #[test]
  fn collect_reachable_program_defs_orders_extra_exprs_and_defs_deterministically() {
    let make_program = |reverse: bool| -> program::CompiledProgram {
      let mut helper_struct = CalcitStructDef::from_fields(EdnTag::new("Widget"), vec![EdnTag::new("id")]);
      helper_struct.definition_ref = Some(Arc::from("app.types/Widget"));
      helper_struct.impls = vec![Arc::new(CalcitImpl {
        name: EdnTag::new("WidgetOps"),
        origin: None,
        fields: Arc::new(vec![EdnTag::new("zebra"), EdnTag::new("alpha")]),
        values: Arc::new(vec![fn_import("app.helpers", "z-fn"), fn_import("app.helpers", "a-fn")]),
      })];
      let main_body = Calcit::from(vec![
        Calcit::Method(Arc::from("zebra"), MethodKind::Invoke(DYNAMIC_TYPE.clone())),
        Calcit::Method(Arc::from("alpha"), MethodKind::Invoke(DYNAMIC_TYPE.clone())),
        Calcit::StructDef(Arc::new(helper_struct.clone())),
      ]);
      let mut main_def = dummy_compiled_def(20, program::CompiledDefKind::Fn, main_body);
      main_def.deps = vec![program::DefId(22), program::DefId(21)];

      let mut ns_entries = vec![
        (
          Arc::from("app.main"),
          program::CompiledFileData {
            defs: HashMap::from([(Arc::from("main!"), main_def)]),
          },
        ),
        (
          Arc::from("app.helpers"),
          program::CompiledFileData {
            defs: HashMap::from([
              (
                Arc::from("z-fn"),
                dummy_compiled_def(21, program::CompiledDefKind::Fn, Calcit::Number(1.0)),
              ),
              (
                Arc::from("a-fn"),
                dummy_compiled_def(22, program::CompiledDefKind::Fn, Calcit::Number(2.0)),
              ),
            ]),
          },
        ),
        (
          Arc::from("app.types"),
          program::CompiledFileData {
            defs: HashMap::from([(
              Arc::from("Widget"),
              dummy_compiled_def(23, program::CompiledDefKind::Value, Calcit::StructDef(Arc::new(helper_struct))),
            )]),
          },
        ),
      ];
      if reverse {
        ns_entries.reverse();
      }
      ns_entries.into_iter().collect()
    };

    let cfg = ReachabilityConfig {
      init_ns: "app.main",
      init_def: "main!",
      target: WasmTarget::Core,
      extra_imports: &[],
      wasi_command_component: false,
      wasi_http_proxy: false,
      wasi_command_read_dir: false,
    };
    let forward = collect_reachable_program_defs(&make_program(false), cfg);
    let reverse = collect_reachable_program_defs(
      &make_program(true),
      ReachabilityConfig {
        init_ns: "app.main",
        init_def: "main!",
        target: WasmTarget::Core,
        extra_imports: &[],
        wasi_command_component: false,
        wasi_http_proxy: false,
        wasi_command_read_dir: false,
      },
    );

    let mut f_defs: Vec<_> = forward.defs.into_iter().collect();
    let mut r_defs: Vec<_> = reverse.defs.into_iter().collect();
    f_defs.sort();
    r_defs.sort();
    assert_eq!(f_defs, r_defs);
    assert_eq!(forward.extra_exprs, reverse.extra_exprs);
  }
}
