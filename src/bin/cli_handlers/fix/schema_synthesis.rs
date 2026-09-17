use super::*;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SchemaEvidenceCandidate {
  pub diagnostic_code: &'static str,
  pub definition: String,
  pub confidence: &'static str,
  pub declared: Value,
  pub candidate: Option<Value>,
  pub unresolved_slots: Vec<String>,
  pub evidence: Vec<Value>,
  pub affected_usages: Vec<String>,
  pub contract_status: &'static str,
}

fn normalize_inferred_schema(annotation: &CalcitTypeAnnotation) -> std::sync::Arc<CalcitTypeAnnotation> {
  use CalcitTypeAnnotation as Type;
  let normalized = match annotation {
    Type::List(inner) => Type::List(normalize_inferred_schema(inner)),
    Type::Map(key, value) => Type::Map(normalize_inferred_schema(key), normalize_inferred_schema(value)),
    Type::Set(inner) => Type::Set(normalize_inferred_schema(inner)),
    Type::Ref(inner) => Type::Ref(normalize_inferred_schema(inner)),
    Type::Optional(inner) => Type::Optional(normalize_inferred_schema(inner)),
    Type::JsNullish(inner) => Type::JsNullish(normalize_inferred_schema(inner)),
    Type::Variadic(inner) => Type::Variadic(normalize_inferred_schema(inner)),
    Type::Fn(signature) => {
      let mut normalized = signature.as_ref().clone();
      normalized.arg_types = normalized.arg_types.iter().map(|arg| normalize_inferred_schema(arg)).collect();
      normalized.return_type = normalize_inferred_schema(&normalized.return_type);
      normalized.rest_type = normalized.rest_type.as_ref().map(|rest| normalize_inferred_schema(rest));
      Type::Fn(std::sync::Arc::new(normalized))
    }
    Type::Struct(definition, args) => Type::TypeRef(
      std::sync::Arc::from(definition.name.ref_str()),
      std::sync::Arc::new(args.iter().map(|arg| normalize_inferred_schema(arg)).collect()),
    ),
    Type::StructValue(definition) => Type::TypeRef(std::sync::Arc::from(definition.name.ref_str()), std::sync::Arc::new(vec![])),
    Type::Enum(definition, args) => Type::TypeRef(
      std::sync::Arc::from(definition.name().ref_str()),
      std::sync::Arc::new(args.iter().map(|arg| normalize_inferred_schema(arg)).collect()),
    ),
    Type::EnumValue(definition) => Type::TypeRef(std::sync::Arc::from(definition.name().ref_str()), std::sync::Arc::new(vec![])),
    other => other.clone(),
  };
  std::sync::Arc::new(normalized)
}

fn refine_schema_holes(
  declared: &std::sync::Arc<CalcitTypeAnnotation>,
  inferred: &std::sync::Arc<CalcitTypeAnnotation>,
) -> std::sync::Arc<CalcitTypeAnnotation> {
  use CalcitTypeAnnotation as Type;
  if matches!(declared.as_ref(), Type::Dynamic | Type::DynFn) {
    return normalize_inferred_schema(inferred);
  }
  let refined = match (declared.as_ref(), inferred.as_ref()) {
    (Type::List(current), Type::List(candidate)) => Type::List(refine_schema_holes(current, candidate)),
    (Type::Set(current), Type::Set(candidate)) => Type::Set(refine_schema_holes(current, candidate)),
    (Type::Ref(current), Type::Ref(candidate)) => Type::Ref(refine_schema_holes(current, candidate)),
    (Type::Optional(current), Type::Optional(candidate)) => Type::Optional(refine_schema_holes(current, candidate)),
    (Type::JsNullish(current), Type::JsNullish(candidate)) => Type::JsNullish(refine_schema_holes(current, candidate)),
    (Type::Variadic(current), Type::Variadic(candidate)) => Type::Variadic(refine_schema_holes(current, candidate)),
    (Type::Map(current_key, current_value), Type::Map(candidate_key, candidate_value)) => Type::Map(
      refine_schema_holes(current_key, candidate_key),
      refine_schema_holes(current_value, candidate_value),
    ),
    (Type::Fn(current), Type::Fn(candidate))
      if current.arg_types.len() == candidate.arg_types.len()
        && current.rest_type.is_some() == candidate.rest_type.is_some()
        && current.fn_kind == candidate.fn_kind =>
    {
      let mut signature = current.as_ref().clone();
      signature.arg_types = current
        .arg_types
        .iter()
        .zip(candidate.arg_types.iter())
        .map(|(declared, inferred)| refine_schema_holes(declared, inferred))
        .collect();
      signature.return_type = refine_schema_holes(&current.return_type, &candidate.return_type);
      signature.rest_type = current
        .rest_type
        .as_ref()
        .zip(candidate.rest_type.as_ref())
        .map(|(declared, inferred)| refine_schema_holes(declared, inferred));
      Type::Fn(std::sync::Arc::new(signature))
    }
    (Type::TypeRef(current_name, current_args), Type::TypeRef(candidate_name, candidate_args))
      if current_name == candidate_name && current_args.len() == candidate_args.len() =>
    {
      Type::TypeRef(
        current_name.clone(),
        std::sync::Arc::new(
          current_args
            .iter()
            .zip(candidate_args.iter())
            .map(|(declared, inferred)| refine_schema_holes(declared, inferred))
            .collect(),
        ),
      )
    }
    _ => return declared.clone(),
  };
  std::sync::Arc::new(refined)
}

fn collect_dynamic_schema_paths(annotation: &CalcitTypeAnnotation, path: &str, paths: &mut Vec<String>) {
  use CalcitTypeAnnotation as Type;
  match annotation {
    Type::Dynamic | Type::DynFn => paths.push(path.to_owned()),
    Type::List(inner)
    | Type::Set(inner)
    | Type::Ref(inner)
    | Type::Optional(inner)
    | Type::JsNullish(inner)
    | Type::Variadic(inner) => collect_dynamic_schema_paths(inner, &format!("{path}.item"), paths),
    Type::Map(key, value) => {
      collect_dynamic_schema_paths(key, &format!("{path}.key"), paths);
      collect_dynamic_schema_paths(value, &format!("{path}.value"), paths);
    }
    Type::Fn(signature) => {
      for (index, arg) in signature.arg_types.iter().enumerate() {
        collect_dynamic_schema_paths(arg, &format!("{path}.args.{index}"), paths);
      }
      if let Some(rest) = &signature.rest_type {
        collect_dynamic_schema_paths(rest, &format!("{path}.rest"), paths);
      }
      collect_dynamic_schema_paths(&signature.return_type, &format!("{path}.return"), paths);
    }
    Type::Struct(_, args) | Type::Enum(_, args) | Type::TypeRef(_, args) => {
      for (index, arg) in args.iter().enumerate() {
        collect_dynamic_schema_paths(arg, &format!("{path}.type-args.{index}"), paths);
      }
    }
    _ => {}
  }
}

fn collect_unbound_schema_paths(
  annotation: &CalcitTypeAnnotation,
  path: &str,
  bound_generics: &HashSet<Arc<str>>,
  paths: &mut Vec<String>,
) {
  use CalcitTypeAnnotation as Type;
  match annotation {
    Type::TypeVar(name) if !bound_generics.contains(name) => paths.push(path.to_owned()),
    Type::TypeSlot(_) => paths.push(path.to_owned()),
    Type::List(inner)
    | Type::Set(inner)
    | Type::Ref(inner)
    | Type::Optional(inner)
    | Type::JsNullish(inner)
    | Type::Variadic(inner) => collect_unbound_schema_paths(inner, &format!("{path}.item"), bound_generics, paths),
    Type::Map(key, value) => {
      collect_unbound_schema_paths(key, &format!("{path}.key"), bound_generics, paths);
      collect_unbound_schema_paths(value, &format!("{path}.value"), bound_generics, paths);
    }
    Type::Fn(signature) => {
      let mut nested_generics = bound_generics.clone();
      nested_generics.extend(signature.generics.iter().cloned());
      for (index, arg) in signature.arg_types.iter().enumerate() {
        collect_unbound_schema_paths(arg, &format!("{path}.args.{index}"), &nested_generics, paths);
      }
      if let Some(rest) = &signature.rest_type {
        collect_unbound_schema_paths(rest, &format!("{path}.rest"), &nested_generics, paths);
      }
      collect_unbound_schema_paths(&signature.return_type, &format!("{path}.return"), &nested_generics, paths);
    }
    Type::Struct(_, args) | Type::Enum(_, args) | Type::TypeRef(_, args) => {
      for (index, arg) in args.iter().enumerate() {
        collect_unbound_schema_paths(arg, &format!("{path}.type-args.{index}"), bound_generics, paths);
      }
    }
    _ => {}
  }
}

fn collect_compiled_refinement_evidence(
  declared: &CalcitTypeAnnotation,
  inferred: &CalcitTypeAnnotation,
  path: &str,
  evidence: &mut Vec<Value>,
) {
  use CalcitTypeAnnotation as Type;
  if matches!(declared, Type::Dynamic | Type::DynFn) {
    match inferred {
      Type::Dynamic | Type::DynFn => {}
      Type::List(inner)
      | Type::Set(inner)
      | Type::Ref(inner)
      | Type::Optional(inner)
      | Type::JsNullish(inner)
      | Type::Variadic(inner) => collect_compiled_refinement_evidence(&Type::Dynamic, inner, &format!("{path}.item"), evidence),
      Type::Map(key, value) => {
        collect_compiled_refinement_evidence(&Type::Dynamic, key, &format!("{path}.key"), evidence);
        collect_compiled_refinement_evidence(&Type::Dynamic, value, &format!("{path}.value"), evidence);
      }
      Type::Fn(signature) => {
        for (index, arg) in signature.arg_types.iter().enumerate() {
          collect_compiled_refinement_evidence(&Type::Dynamic, arg, &format!("{path}.args.{index}"), evidence);
        }
        if let Some(rest) = &signature.rest_type {
          collect_compiled_refinement_evidence(&Type::Dynamic, rest, &format!("{path}.rest"), evidence);
        }
        collect_compiled_refinement_evidence(&Type::Dynamic, &signature.return_type, &format!("{path}.return"), evidence);
      }
      Type::Struct(_, args) | Type::Enum(_, args) | Type::TypeRef(_, args) if !args.is_empty() => {
        for (index, arg) in args.iter().enumerate() {
          collect_compiled_refinement_evidence(&Type::Dynamic, arg, &format!("{path}.type-args.{index}"), evidence);
        }
      }
      _ => evidence.push(serde_json::json!({
        "kind": "compiled-expression-type",
        "slot": path,
        "inferred": inferred.to_brief_string(),
      })),
    }
    return;
  }

  match (declared, inferred) {
    (Type::List(current), Type::List(candidate))
    | (Type::Set(current), Type::Set(candidate))
    | (Type::Ref(current), Type::Ref(candidate))
    | (Type::Optional(current), Type::Optional(candidate))
    | (Type::JsNullish(current), Type::JsNullish(candidate))
    | (Type::Variadic(current), Type::Variadic(candidate)) => {
      collect_compiled_refinement_evidence(current, candidate, &format!("{path}.item"), evidence);
    }
    (Type::Map(current_key, current_value), Type::Map(candidate_key, candidate_value)) => {
      collect_compiled_refinement_evidence(current_key, candidate_key, &format!("{path}.key"), evidence);
      collect_compiled_refinement_evidence(current_value, candidate_value, &format!("{path}.value"), evidence);
    }
    (Type::Fn(current), Type::Fn(candidate)) if current.arg_types.len() == candidate.arg_types.len() => {
      for (index, (declared, inferred)) in current.arg_types.iter().zip(candidate.arg_types.iter()).enumerate() {
        collect_compiled_refinement_evidence(declared, inferred, &format!("{path}.args.{index}"), evidence);
      }
      collect_compiled_refinement_evidence(&current.return_type, &candidate.return_type, &format!("{path}.return"), evidence);
      if let (Some(declared), Some(inferred)) = (&current.rest_type, &candidate.rest_type) {
        collect_compiled_refinement_evidence(declared, inferred, &format!("{path}.rest"), evidence);
      }
    }
    (Type::TypeRef(current_name, current_args), Type::TypeRef(candidate_name, candidate_args))
      if current_name == candidate_name && current_args.len() == candidate_args.len() =>
    {
      for (index, (declared, inferred)) in current_args.iter().zip(candidate_args.iter()).enumerate() {
        collect_compiled_refinement_evidence(declared, inferred, &format!("{path}.type-args.{index}"), evidence);
      }
    }
    _ => {}
  }
}

fn merge_callsite_argument_evidence(
  inferred: std::sync::Arc<CalcitTypeAnnotation>,
  snapshot: &Snapshot,
  project_definitions: &[(String, String)],
  target_ns: &str,
  target_def: &str,
  tolerate_unavailable_owners: bool,
) -> Result<(std::sync::Arc<CalcitTypeAnnotation>, Vec<Value>), String> {
  struct StrictTypesGuard(bool);

  impl Drop for StrictTypesGuard {
    fn drop(&mut self) {
      runner::preprocess::set_strict_types(self.0);
    }
  }

  let CalcitTypeAnnotation::Fn(signature) = inferred.as_ref() else {
    return Ok((inferred, vec![]));
  };
  if signature.arg_types.is_empty() {
    return Ok((inferred, vec![]));
  }

  let warnings = RefCell::new(Vec::new());
  let _strict_types_guard = StrictTypesGuard(runner::preprocess::is_strict_types_enabled());
  runner::preprocess::set_strict_types(false);
  let mut candidates: Vec<Option<std::sync::Arc<CalcitTypeAnnotation>>> = vec![None; signature.arg_types.len()];
  let mut paths: Vec<Vec<String>> = vec![vec![]; signature.arg_types.len()];
  let mut blocked = vec![false; signature.arg_types.len()];
  let mut observed: Vec<HashSet<String>> = vec![HashSet::new(); signature.arg_types.len()];
  for (owner_ns, owner_def) in project_definitions {
    if is_sample_namespace(owner_ns) {
      continue;
    }
    let source_entry = snapshot
      .files
      .get(owner_ns)
      .and_then(|file| file.defs.get(owner_def))
      .ok_or_else(|| format!("Project definition `{owner_ns}/{owner_def}` is missing from the source snapshot."))?;
    let owner_is_macro = list_head(&source_entry.code) == Some("defmacro");
    if let Err(failure) = runner::preprocess::compile_source_def_for_snapshot(owner_ns, owner_def, &warnings, &CallStackList::default())
    {
      if tolerate_unavailable_owners {
        continue;
      }
      return Err(failure.msg);
    }
    let Some(compiled) = program::lookup_compiled_def(owner_ns, owner_def) else {
      continue;
    };
    let usages = match runner::preprocess::trace_definition_source_usages(owner_ns, owner_def, &warnings, &CallStackList::default()) {
      Ok(usages) => usages,
      Err(_) if tolerate_unavailable_owners => continue,
      Err(failure) => return Err(failure.msg),
    };
    for usage in usages {
      if usage.target_ns.as_ref() != target_ns || usage.target_def.as_ref() != target_def {
        continue;
      }
      if owner_is_macro {
        blocked.fill(true);
        continue;
      }
      let Some(location) = usage.location else {
        blocked.fill(true);
        continue;
      };
      if !usage.macro_origin.is_empty() || location.ns.as_ref() != owner_ns || location.def.as_ref() != owner_def {
        blocked.fill(true);
        continue;
      }
      let mut call_path = location.coord.iter().map(|value| usize::from(*value)).collect::<Vec<_>>();
      if call_path.pop() != Some(0) {
        blocked.fill(true);
        continue;
      }
      let source_call = navigate_to_path(&source_entry.code, &call_path)?;
      let Cirru::List(source_items) = source_call else {
        blocked.fill(true);
        continue;
      };
      if source_items.len().saturating_sub(1) != signature.arg_types.len() {
        blocked.fill(true);
        continue;
      }
      for (index, source_argument) in source_items.iter().skip(1).enumerate() {
        let mut argument_path = call_path.clone();
        argument_path.push(index + 1);
        let source_argument = code_to_calcit(
          source_argument,
          owner_ns,
          owner_def,
          argument_path
            .iter()
            .map(|value| {
              u16::try_from(*value).map_err(|_| format!("Source path index `{value}` exceeds the compiler coordinate range."))
            })
            .collect::<Result<Vec<_>, _>>()?,
        )?;
        let processed_argument = super::super::query::find_preprocessed_node_at_path(
          &compiled.preprocessed_code,
          owner_ns,
          owner_def,
          &argument_path,
          matches!(source_items[index + 1], Cirru::List(_)),
        );
        let Some(candidate) = super::super::query::infer_type_at_target(&source_argument, processed_argument, owner_ns)
          .map(|value| normalize_inferred_schema(&value))
        else {
          blocked[index] = true;
          continue;
        };
        if matches!(candidate.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn) {
          blocked[index] = true;
          continue;
        }
        observed[index].insert(candidate.to_brief_string());
        match &candidates[index] {
          Some(current) if current != &candidate => blocked[index] = true,
          Some(_) => {}
          None => candidates[index] = Some(candidate),
        }
        paths[index].push(format!("{owner_ns}/{owner_def}{}", format_path(&call_path)));
      }
    }
  }

  let mut updated = signature.as_ref().clone();
  let mut evidence = Vec::new();
  for (index, current) in updated.arg_types.iter_mut().enumerate() {
    if !matches!(current.as_ref(), CalcitTypeAnnotation::Dynamic | CalcitTypeAnnotation::DynFn) {
      continue;
    }
    if blocked[index] {
      let mut inferred = observed[index].iter().cloned().collect::<Vec<_>>();
      inferred.sort();
      evidence.push(serde_json::json!({
        "kind": if inferred.len() > 1 { "conflicting-callsite-arguments" } else { "unsafe-callsite-evidence" },
        "slot": format!("schema.args.{index}"),
        "inferred": inferred,
        "calls": paths[index],
      }));
      continue;
    }
    let Some(candidate) = candidates[index].clone() else {
      continue;
    };
    *current = candidate.clone();
    evidence.push(serde_json::json!({
      "kind": "resolved-callsite-arguments",
      "slot": format!("schema.args.{index}"),
      "inferred": candidate.to_brief_string(),
      "calls": paths[index],
    }));
  }
  Ok((
    std::sync::Arc::new(CalcitTypeAnnotation::Fn(std::sync::Arc::new(updated))),
    evidence,
  ))
}

fn schema_candidate_for_definition(
  snapshot: &Snapshot,
  project_definitions: &[(String, String)],
  namespace: &str,
  definition: &str,
  tolerate_unavailable_owners: bool,
) -> Result<Option<SchemaEvidenceCandidate>, String> {
  let entry = snapshot
    .files
    .get(namespace)
    .and_then(|file| file.defs.get(definition))
    .ok_or_else(|| format!("Schema evidence target `{namespace}/{definition}` does not exist."))?;
  if matches!(
    list_head(&entry.code),
    Some("defmacro" | "defstruct" | "defenum" | "deftrait" | "defimpl")
  ) {
    return Ok(None);
  }

  let original_node = schema_edn_to_source_node(&calcit::snapshot::schema_annotation_to_edn(entry.schema.as_ref()))?;
  let Some(inferred) = runner::preprocess::infer_compiled_definition_implementation_type(namespace, definition) else {
    return Ok(Some(SchemaEvidenceCandidate {
      diagnostic_code: "I_SCHEMA_CANDIDATE_EVIDENCE",
      definition: format!("{namespace}/{definition}"),
      confidence: "boundary-unknown",
      declared: quoted_json(&original_node),
      candidate: None,
      unresolved_slots: vec!["schema".to_owned()],
      evidence: vec![serde_json::json!({
        "kind": "compiled-type-unavailable",
        "target": format!("{namespace}/{definition}"),
      })],
      affected_usages: vec![],
      contract_status: "review-required",
    }));
  };
  let inferred = normalize_inferred_schema(&inferred);
  let mut compiled_evidence = Vec::new();
  collect_compiled_refinement_evidence(entry.schema.as_ref(), inferred.as_ref(), "schema", &mut compiled_evidence);
  let (inferred, mut callsite_evidence) = merge_callsite_argument_evidence(
    inferred,
    snapshot,
    project_definitions,
    namespace,
    definition,
    tolerate_unavailable_owners,
  )?;
  let candidate = refine_schema_holes(&entry.schema, &inferred);
  if candidate == entry.schema {
    return Ok(None);
  }

  let replacement_node = schema_edn_to_source_node(&calcit::snapshot::schema_annotation_to_edn(candidate.as_ref()))?;
  let mut unresolved_slots = Vec::new();
  collect_dynamic_schema_paths(candidate.as_ref(), "schema", &mut unresolved_slots);
  collect_unbound_schema_paths(candidate.as_ref(), "schema", &HashSet::new(), &mut unresolved_slots);
  unresolved_slots.sort();
  unresolved_slots.dedup();
  callsite_evidence.insert(
    0,
    serde_json::json!({
      "kind": "compiled-type-inference",
      "target": format!("{namespace}/{definition}"),
      "inferred": inferred.to_brief_string(),
      "unresolved_slots": unresolved_slots,
    }),
  );
  callsite_evidence.splice(1..1, compiled_evidence);
  let has_conflict = callsite_evidence
    .iter()
    .any(|item| item["kind"] == "conflicting-callsite-arguments");
  let usage_derived = callsite_evidence.iter().any(|item| item["kind"] == "resolved-callsite-arguments");
  let confidence = if has_conflict {
    "conflict"
  } else if !unresolved_slots.is_empty() {
    "boundary-unknown"
  } else if usage_derived {
    "usage-derived"
  } else {
    "exact"
  };
  let mut affected_usages = callsite_evidence
    .iter()
    .filter_map(|item| item.get("calls").and_then(Value::as_array))
    .flatten()
    .filter_map(Value::as_str)
    .map(str::to_owned)
    .collect::<Vec<_>>();
  affected_usages.sort();
  affected_usages.dedup();
  Ok(Some(SchemaEvidenceCandidate {
    diagnostic_code: "I_SCHEMA_CANDIDATE_EVIDENCE",
    definition: format!("{namespace}/{definition}"),
    confidence,
    declared: quoted_json(&original_node),
    candidate: Some(quoted_json(&replacement_node)),
    unresolved_slots,
    evidence: callsite_evidence,
    affected_usages,
    contract_status: "review-required",
  }))
}

pub(crate) fn collect_schema_evidence_candidates(
  snapshot: &Snapshot,
  project_definitions: &[(String, String)],
  targets: &[(String, String)],
) -> Result<Vec<SchemaEvidenceCandidate>, String> {
  let mut candidates = Vec::new();
  for (namespace, definition) in targets {
    if let Some(candidate) = schema_candidate_for_definition(snapshot, project_definitions, namespace, definition, true)? {
      candidates.push(candidate);
    }
  }
  candidates.sort_by(|left, right| left.definition.cmp(&right.definition));
  Ok(candidates)
}

/// Test and example namespaces contain executable samples, not production-wide
/// constraints. Attached tests/examples are validated separately after staging.
fn is_sample_namespace(namespace: &str) -> bool {
  namespace
    .split('.')
    .any(|segment| matches!(segment, "test" | "tests" | "example" | "examples"))
}

fn validate_attached_sources_against_synthesized_schema(
  snapshot: &Snapshot,
  project_definitions: &[(String, String)],
  target_ns: &str,
  target_def: &str,
) -> Result<(), String> {
  for (owner_ns, owner_def) in project_definitions {
    let entry = snapshot
      .files
      .get(owner_ns)
      .and_then(|file| file.defs.get(owner_def))
      .ok_or_else(|| format!("Project definition `{owner_ns}/{owner_def}` is missing from the staged snapshot."))?;
    let mut attached = entry
      .tests
      .iter()
      .enumerate()
      .map(|(index, test)| {
        (
          format!("test `{}`", test.name),
          format!("&calcit:schema-test:{owner_def}:{index}"),
          &test.code,
        )
      })
      .collect::<Vec<_>>();
    attached.extend(entry.examples.iter().enumerate().map(|(index, example)| {
      (
        format!("example {index}"),
        format!("&calcit:schema-example:{owner_def}:{index}"),
        example,
      )
    }));
    for (label, synthetic_def, source) in attached {
      if !cirru_contains_target_reference(source, owner_ns, target_ns, target_def) {
        continue;
      }
      let wrapper = Cirru::List(vec![Cirru::leaf("fn"), Cirru::List(vec![]), source.clone()]);
      let parsed = code_to_calcit(&wrapper, owner_ns, &synthetic_def, vec![])
        .map_err(|error| format!("{owner_ns}/{owner_def} {label}: failed to parse attached source: {error}"))?;
      let warnings = RefCell::new(Vec::new());
      runner::preprocess::trace_source_usages(&parsed, owner_ns, &synthetic_def, &warnings, &CallStackList::default()).map_err(
        |failure| {
          format!(
            "{owner_ns}/{owner_def} {label}: synthesized schema for `{target_ns}/{target_def}` does not validate: {}",
            failure.msg
          )
        },
      )?;
      let warnings = warnings.into_inner();
      if !warnings.is_empty() {
        return Err(format!(
          "{owner_ns}/{owner_def} {label}: synthesized schema for `{target_ns}/{target_def}` produced strict warnings:\n{}",
          warnings.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
        ));
      }
    }
  }
  Ok(())
}

pub(super) fn plan_schema_synthesis(
  options: &FixCommand,
  snapshot: &Snapshot,
  snapshot_file: &str,
  project_definitions: &[(String, String)],
) -> Result<Vec<FixSuggestion>, String> {
  let namespace = options.ns.as_deref().expect("schema synthesis requires namespace");
  let definition = options.definition.as_deref().expect("schema synthesis requires definition");
  let entry = snapshot
    .files
    .get(namespace)
    .and_then(|file| file.defs.get(definition))
    .ok_or_else(|| format!("Schema synthesis target `{namespace}/{definition}` does not exist."))?;
  if matches!(
    list_head(&entry.code),
    Some("defmacro" | "defstruct" | "defenum" | "deftrait" | "defimpl")
  ) {
    return Err(format!(
      "Schema synthesis for `{namespace}/{definition}` is limited to runtime values and functions; declaration and macro forms require explicit contracts."
    ));
  }

  let Some(evidence_candidate) = schema_candidate_for_definition(snapshot, project_definitions, namespace, definition, false)? else {
    if std::env::var("CALCIT_FIX_VALIDATE_ONLY").as_deref() == Ok("1") {
      validate_attached_sources_against_synthesized_schema(snapshot, project_definitions, namespace, definition)?;
    }
    return Ok(vec![]);
  };
  let Some(candidate_json) = evidence_candidate.candidate.as_ref() else {
    return Err(format!(
      "Schema synthesis could not recover static implementation evidence for `{namespace}/{definition}`; no schema was guessed."
    ));
  };
  if std::env::var("CALCIT_FIX_VALIDATE_ONLY").as_deref() == Ok("1") {
    validate_attached_sources_against_synthesized_schema(snapshot, project_definitions, namespace, definition)?;
  }

  let original_node = schema_edn_to_source_node(&calcit::snapshot::schema_annotation_to_edn(entry.schema.as_ref()))?;
  let replacement_node = json_value_to_cirru(
    candidate_json
      .get("value")
      .ok_or_else(|| format!("Schema evidence for `{namespace}/{definition}` omitted the Cirru candidate value."))?,
  )?;
  let unresolved = evidence_candidate.unresolved_slots;
  let machine_applicable = unresolved.is_empty();
  Ok(vec![FixSuggestion {
    rule_id: SYNTHESIZE_SCHEMA_RULE,
    diagnostic_code: SYNTHESIZE_SCHEMA_DIAGNOSTIC,
    semantic_layer: "surface",
    source_file: snapshot_file.to_owned(),
    definition: format!("{namespace}/{definition}"),
    path: "schema".to_owned(),
    fingerprint: node_fingerprint(&original_node),
    origin_chain: evidence_candidate.evidence,
    original: quoted_json(&original_node),
    replacement: Some(quoted_json(&replacement_node)),
    applicability: if machine_applicable { "machine-applicable" } else { "needs-review" },
    message: if machine_applicable {
      "Fill schema holes from the existing preprocessor and bottom-up inference result.".to_owned()
    } else {
      format!(
        "The inferred candidate preserves unresolved slots at {}; review or add constraints before applying.",
        unresolved.join(", ")
      )
    },
    target_path: vec![],
    operation: machine_applicable.then(|| FixOperation::ReplaceSchema {
      code: format_quoted_nodes(std::slice::from_ref(&replacement_node)).expect("formatted inferred schema"),
    }),
  }])
}
