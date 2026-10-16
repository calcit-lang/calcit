//! Explicit bridge for legacy nil consumers of absence-reporting primitives.
//! Public results are matched without confusing a present nil with absence or
//! turning a lazy fallback into an eager function argument. Other uses require
//! review rather than guessing a producer or application return contract.

use super::case_default::{ContextSubject, MacroHeadCache, unstable_macro_context};
use super::*;
use calcit::calcit::NullableOrigin;

#[derive(Clone, Copy)]
enum Family {
  Option,
  Result,
  Map,
  Set,
}

#[derive(Clone, Copy)]
enum Domain {
  Map,
  List,
  Set,
  String,
  Enum,
  Struct,
  Impl,
  Traversal,
}

#[derive(Clone, Copy)]
struct Entry {
  public: &'static str,
  arity: usize,
  domain: Domain,
  family: Family,
}

/// One migration table, not a family of parallel public APIs or presets.
fn entry(name: &str) -> Option<Entry> {
  let name = name.strip_prefix("calcit.core/").unwrap_or(name);
  let (public, arity, domain, family) = match name {
    "&map:get" => ("get", 2, Domain::Map, Family::Option),
    "&list:first" => ("first", 1, Domain::List, Family::Option),
    "&list:last" => ("last", 1, Domain::List, Family::Option),
    "&str:first" => ("first", 1, Domain::String, Family::Option),
    "&str:nth" => ("nth", 2, Domain::String, Family::Option),
    "&get-env" => ("get-env", 1, Domain::String, Family::Option),
    "&get-in" => ("get-in", 2, Domain::Traversal, Family::Result),
    "&enum:definition" => ("enum-definition", 1, Domain::Enum, Family::Option),
    "&struct:definition" => ("struct-definition", 1, Domain::Struct, Family::Option),
    "&impl:origin" => ("impl-origin", 1, Domain::Impl, Family::Option),
    "&parse-float" => ("parse-float", 1, Domain::String, Family::Result),
    "&map:destruct" => ("destruct-map", 1, Domain::Map, Family::Map),
    "&set:destruct" => ("destruct-set", 1, Domain::Set, Family::Set),
    _ => return None,
  };
  Some(Entry {
    public,
    arity,
    domain,
    family,
  })
}

#[derive(Clone, Copy)]
enum Consumer {
  Nil,
  NonNil,
  Either,
}

type ProvenConsumer = (Entry, Consumer);
type PlannedConsumers = BTreeMap<Vec<usize>, (Option<ProvenConsumer>, String)>;

pub(super) fn plan(snapshot: &Snapshot, snapshot_file: &str, definitions: &[(String, String)]) -> Result<Vec<FixSuggestion>, String> {
  let mut suggestions = Vec::new();
  for (namespace, definition) in definitions {
    let source = &snapshot.files[namespace].defs[definition].code;
    suggestions.extend(plan_source(snapshot, snapshot_file, namespace, definition, source)?);
  }
  Ok(suggestions)
}

/// The attached-source workflow supplies the same source wrapper and proofs.
pub(super) fn plan_source(
  snapshot: &Snapshot,
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
) -> Result<Vec<FixSuggestion>, String> {
  // The old source can already contradict the new primitive return contracts.
  // Read its receiver evidence in migration mode; writes still require the
  // shared strict staged subprocess and never trust missing trace evidence.
  with_legacy_migration_mode(|| plan_source_with_legacy_evidence(snapshot, snapshot_file, namespace, definition, source))
}

fn plan_source_with_legacy_evidence(
  snapshot: &Snapshot,
  snapshot_file: &str,
  namespace: &str,
  definition: &str,
  source: &Cirru,
) -> Result<Vec<FixSuggestion>, String> {
  let mut candidates = Vec::new();
  collect(source, &mut Vec::new(), false, &mut candidates);
  if candidates.is_empty() {
    return Ok(Vec::new());
  }
  let mut locals = HashSet::new();
  collect_potential_local_bindings(source, &mut locals);
  let parsed = code_to_calcit(source, namespace, definition, vec![]).map_err(|error| error.to_string())?;
  let expressions = runner::preprocess::trace_snapshot_source_expressions(
    source,
    namespace,
    definition,
    &RefCell::new(vec![]),
    &CallStackList::default(),
  )
  .unwrap_or_default();
  let usages =
    runner::preprocess::trace_source_usages(&parsed, namespace, definition, &RefCell::new(vec![]), &CallStackList::default())
      .unwrap_or_default();
  let mut heads = MacroHeadCache::new();
  let mut used = HashSet::new();
  collect_leaves(source, &mut used);
  let mut planned = PlannedConsumers::new();
  for (leaf_path, template) in candidates {
    let leaf = navigate_to_path(source, &leaf_path)?;
    let name = leaf_value(&leaf).expect("collected primitive leaf");
    let api = entry(name).expect("collected primitive");
    let call_path = leaf_path.strip_suffix(&[0]);
    let owner = call_path.map_or_else(|| leaf_path.clone(), <[usize]>::to_vec);
    let mut target = owner.clone();
    let mut consumer = None;
    if let Some(call_path) = call_path
      && let Some((&1, parent)) = call_path.split_last()
      && let Ok(Cirru::List(items)) = navigate_to_path(source, parent)
      && let Some(head) = items.first().and_then(leaf_value)
    {
      let qualified = head.starts_with("calcit.core/");
      let short = head.strip_prefix("calcit.core/").unwrap_or(head);
      let unshadowed =
        qualified || (!locals.contains(short) && (namespace == "calcit.core" || !namespace_binds_name(snapshot, namespace, short)));
      consumer = match (short, items.len(), unshadowed) {
        ("either", length, true) if length >= 2 => Some(Consumer::Either),
        // nil? and non-nil? each have one argument: head plus subject.
        ("nil?", 2, true) => Some(Consumer::Nil),
        ("non-nil?", 2, true) => Some(Consumer::NonNil),
        _ => None,
      };
      if consumer.is_some() {
        target = parent.to_vec();
      }
    }
    let reason = if template || list_head(source) == Some("defmacro") {
      Some("The primitive occurs in macro/template source; review its expansion sites.".to_owned())
    } else if call_path.is_none() {
      Some("The primitive is a first-class value; review callable identity and its consumers.".to_owned())
    } else if consumer.is_none() {
      Some("No direct legacy nil?/non-nil?/either consumer is proven; review the binding and all uses together.".to_owned())
    } else if locals.contains("match") || (namespace != "calcit.core" && namespace_binds_name(snapshot, namespace, "match")) {
      Some("The replacement match syntax is shadowed; review the lexical environment.".to_owned())
    } else if let Some(reason) = unstable_macro_context(
      snapshot,
      snapshot_file,
      namespace,
      source,
      &target,
      ContextSubject { name, transparent: &[] },
      &mut heads,
    ) {
      Some(reason)
    } else {
      check_call(
        source,
        call_path.expect("checked"),
        name,
        api,
        (namespace, definition),
        &expressions,
        &usages,
      )?
    };
    planned.insert(
      target,
      (
        reason.is_none().then(|| (api, consumer.expect("checked"))),
        reason.unwrap_or_default(),
      ),
    );
  }
  let proven = planned
    .iter()
    .filter_map(|(path, (proof, _))| proof.map(|proof| (path.clone(), proof)))
    .collect::<BTreeMap<_, _>>();
  let mut suggestions = Vec::new();
  for (path, (proof, reason)) in planned {
    if proof.is_some() && proven.keys().any(|parent| parent.len() < path.len() && path.starts_with(parent)) {
      continue;
    }
    let original = navigate_to_path(source, &path)?;
    let replacement = proof.map(|_| rewrite(&original, &mut path.clone(), &proven, &mut used));
    let operation = replacement
      .as_ref()
      .map(|replacement| {
        Ok::<_, String>(FixOperation::ReplaceNode {
          original: original.format_one_liner().map_err(|error| error.to_string())?,
          replacement: replacement.format_one_liner().map_err(|error| error.to_string())?,
        })
      })
      .transpose()?;
    suggestions.push(FixSuggestion {
      rule_id: CORE_ABSENCE_RULE,
      diagnostic_code: CORE_ABSENCE_DIAGNOSTIC,
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{namespace}/{definition}"),
      path: format!("code{}", format_path(&path)),
      fingerprint: node_fingerprint(&original),
      origin_chain: vec![serde_json::json!({"kind":"legacy-absence-consumer", "target":"public-typed-contract", "retirement":"after-active-consumer-migration"})],
      original: quoted_json(&original),
      replacement: replacement.as_ref().map(quoted_json),
      applicability: if proof.is_some() { "machine-applicable" } else { "requires-review" },
      message: if proof.is_some() {
        "Match the public result, preserving present nil, lazy fallback and once-only argument evaluation.".to_owned()
      } else { reason },
      target_path: path,
      operation,
    });
  }
  Ok(suggestions)
}

/// Quoted data is not migration code. Templates remain conservative, including
/// nested unquotes; the rule never guesses their runtime lexical environment.
fn collect(node: &Cirru, path: &mut Vec<usize>, template: bool, output: &mut Vec<(Vec<usize>, bool)>) {
  match node {
    Cirru::Leaf(name) if entry(name).is_some() => output.push((path.clone(), template)),
    Cirru::Leaf(_) => {}
    Cirru::List(items) => {
      let head = items
        .first()
        .and_then(leaf_value)
        .map(|name| name.strip_prefix("calcit.core/").unwrap_or(name));
      if matches!(head, Some("quote" | "cirru-quote" | ";")) {
        return;
      }
      let template = template || head == Some("quasiquote");
      for (index, item) in items.iter().enumerate() {
        path.push(index);
        collect(item, path, template, output);
        path.pop();
      }
    }
  }
}

fn collect_leaves(node: &Cirru, output: &mut HashSet<String>) {
  match node {
    Cirru::Leaf(name) => {
      output.insert(name.to_string());
    }
    Cirru::List(items) => {
      for item in items {
        collect_leaves(item, output);
      }
    }
  }
}

fn inferred(
  source: &Cirru,
  namespace: &str,
  definition: &str,
  path: &[usize],
  evidence: &[runner::preprocess::SourceExpressionEvidence],
) -> Option<Arc<CalcitTypeAnnotation>> {
  let matches_location = |item: &&runner::preprocess::SourceExpressionEvidence| {
    item.location.ns.as_ref() == namespace
      && item.location.def.as_ref() == definition
      && item.location.coord.iter().map(|index| usize::from(*index)).eq(path.iter().copied())
  };
  let traced = runner::preprocess::unique_source_expression_at_path(evidence, namespace, definition, path);
  if traced.is_none() && evidence.iter().filter(matches_location).count() > 1 {
    return None;
  }
  let annotation = traced
    .and_then(|item| item.inferred_type.clone())
    .or_else(|| {
      let compiled = program::lookup_compiled_def(namespace, definition)?;
      let node = navigate_to_path(source, path).ok()?;
      super::super::query::find_preprocessed_node_at_path(
        &compiled.preprocessed_code,
        namespace,
        definition,
        path,
        matches!(node, Cirru::List(_)),
      )
      .and_then(runner::preprocess::infer_static_type_from_expr)
    })
    .or_else(|| {
      let node = navigate_to_path(source, path).ok()?;
      let value = code_to_calcit(&node, namespace, definition, vec![]).ok()?;
      // A literal cannot accidentally resolve a same-named global instead of a
      // local. Other missing lexical evidence remains a review requirement.
      let literal = matches!(
        value,
        Calcit::Str(_) | Calcit::Number(_) | Calcit::Bool(_) | Calcit::Nil | Calcit::Tag(_)
      ) || matches!(&value, Calcit::List(items) if matches!(items.first(), Some(Calcit::Proc(proc))
        if matches!(proc.as_ref(), "[]" | "&{}" | "#{}")));
      literal.then(|| runner::preprocess::infer_static_type_from_expr(&value)).flatten()
    })?;
  Some(runner::preprocess::resolve_namespace_type_refs_for_body(annotation, namespace))
}

fn check_call(
  source: &Cirru,
  path: &[usize],
  name: &str,
  api: Entry,
  (namespace, definition): (&str, &str),
  expressions: &[runner::preprocess::SourceExpressionEvidence],
  usages: &[runner::preprocess::ResolvedSourceUsage],
) -> Result<Option<String>, String> {
  let Cirru::List(items) = navigate_to_path(source, path)? else {
    unreachable!("call parent is a list")
  };
  if items.len() != api.arity + 1 {
    return Ok(Some(
      "Review the legacy arity/default argument; no argument or effect may be dropped.".to_owned(),
    ));
  }
  let raw = name.strip_prefix("calcit.core/").unwrap_or(name);
  let reader_proven =
    matches!(code_to_calcit(&items[0], namespace, definition, vec![]), Ok(Calcit::Proc(proc)) if proc.as_ref() == raw);
  let mut head_path = path.to_vec();
  head_path.push(0);
  let mut matching_usages = usages.iter().filter(|usage| {
    usage.location.as_ref().is_some_and(|location| {
      location.ns.as_ref() == namespace
        && location.def.as_ref() == definition
        && location.coord.iter().map(|index| usize::from(*index)).eq(head_path.iter().copied())
    })
  });
  // A macro may duplicate a source coordinate under different bindings. One
  // matching expansion is not enough to authorize all of those expansions.
  let resolves_to_primitive =
    |usage: &runner::preprocess::ResolvedSourceUsage| usage.target_ns.as_ref() == "calcit.core" && usage.target_def.as_ref() == raw;
  let resolved = matching_usages.next().is_some_and(resolves_to_primitive) && matching_usages.all(resolves_to_primitive);
  if !reader_proven && !resolved {
    return Ok(Some("The source head is not proven to resolve to the core primitive.".to_owned()));
  }
  let mut argument_path = path.to_vec();
  argument_path.push(1);
  let annotation = inferred(source, namespace, definition, &argument_path, expressions);
  let domain_proven = annotation.as_deref().is_some_and(|annotation| match (api.domain, annotation) {
    (Domain::Map, CalcitTypeAnnotation::Map(..))
    | (Domain::List, CalcitTypeAnnotation::List(_))
    | (Domain::Set, CalcitTypeAnnotation::Set(_))
    | (Domain::String, CalcitTypeAnnotation::String)
    | (Domain::Enum, CalcitTypeAnnotation::Enum(..) | CalcitTypeAnnotation::EnumValue(_) | CalcitTypeAnnotation::AnonymousEnum)
    | (Domain::Struct, CalcitTypeAnnotation::Struct(..) | CalcitTypeAnnotation::StructValue(_)) => true,
    (Domain::Impl, CalcitTypeAnnotation::Custom(value)) => matches!(value.as_ref(), Calcit::Impl(_)),
    (Domain::Traversal, annotation) => traversal_proven(annotation, &items[2]),
    _ => false,
  });
  if !domain_proven {
    if matches!(api.domain, Domain::Traversal) {
      return Ok(Some(
        "Review the traversal: prove a literal path with compatible lookup steps; Struct fields, open intermediate values and unproven indices cannot use public get-in automatically.".to_owned(),
      ));
    }
    return Ok(Some(
      "The original receiver domain is not proven; a wider public lookup must not hide an invalid input.".to_owned(),
    ));
  }
  if matches!(api.domain, Domain::Traversal) {
    *argument_path.last_mut().expect("argument") = 2;
    if !matches!(
      inferred(source, namespace, definition, &argument_path, expressions).as_deref(),
      Some(CalcitTypeAnnotation::List(_))
    ) {
      return Ok(Some("The traversal path does not prove the original List contract.".to_owned()));
    }
  }
  if raw == "&str:nth"
    && !matches!(code_to_calcit(&items[2], namespace, definition, vec![]), Ok(Calcit::Number(index))
    if index.is_finite() && index.fract() == 0.0 && index >= 0.0 && index <= f64::from(i32::MAX))
  {
    return Ok(Some(
      "Review the string index failure contract; this bridge only proves literal nonnegative i32 indices.".to_owned(),
    ));
  }
  Ok(None)
}

/// Public get-in rejects Struct hops, unlike the legacy raw traversal. Prove
/// every receiver, not just the root: a Map/List may contain a Struct or an
/// unknown intermediate value. A final payload is returned, not traversed.
fn traversal_proven(base: &CalcitTypeAnnotation, path: &Cirru) -> bool {
  let Cirru::List(items) = path else { return false };
  if !matches!(items.first().and_then(leaf_value), Some("[]" | "calcit.core/[]")) {
    return false;
  }
  let keys = &items[1..];
  let mut current = base;
  for (position, key) in keys.iter().enumerate() {
    while let CalcitTypeAnnotation::Nullable(inner, NullableOrigin::Calcit) = current {
      current = inner;
    }
    let safe_index = || {
      leaf_value(key)
        .and_then(|value| value.parse::<f64>().ok())
        .is_some_and(|value| value.is_finite() && value.fract() == 0.0 && value >= 0.0 && value <= f64::from(i32::MAX))
    };
    match current {
      CalcitTypeAnnotation::Nil | CalcitTypeAnnotation::Never => return true,
      CalcitTypeAnnotation::Map(_, inner) => current = inner,
      CalcitTypeAnnotation::List(inner) if safe_index() => current = inner,
      CalcitTypeAnnotation::String if safe_index() => {}
      // The tag is always present. Other enum indices require variant/arity
      // evidence because raw enum indexing can throw where public nth is None.
      CalcitTypeAnnotation::Enum(..) | CalcitTypeAnnotation::EnumValue(_) | CalcitTypeAnnotation::AnonymousEnum => {
        return position + 1 == keys.len() && leaf_value(key).and_then(|value| value.parse::<f64>().ok()) == Some(0.0);
      }
      _ => return false,
    }
  }
  true
}

fn list(items: impl IntoIterator<Item = Cirru>) -> Cirru {
  Cirru::List(items.into_iter().collect())
}
fn leaf(name: &str) -> Cirru {
  Cirru::leaf(name)
}

fn fresh(used: &mut HashSet<String>) -> Cirru {
  for index in 0usize.. {
    let name = format!("calcit-fix-value-{index}");
    if used.insert(name.clone()) {
      return Cirru::leaf(name);
    }
  }
  unreachable!()
}

/// Merge proven nested replacements before building the enclosing match.
fn rewrite(node: &Cirru, path: &mut Vec<usize>, proven: &BTreeMap<Vec<usize>, (Entry, Consumer)>, used: &mut HashSet<String>) -> Cirru {
  let Cirru::List(items) = node else { return node.clone() };
  let rewritten = items
    .iter()
    .enumerate()
    .map(|(index, item)| {
      path.push(index);
      let result = rewrite(item, path, proven, used);
      path.pop();
      result
    })
    .collect::<Vec<_>>();
  let Some((api, consumer)) = proven.get(path) else {
    return Cirru::List(rewritten);
  };
  let Cirru::List(raw) = &rewritten[1] else {
    unreachable!("proven direct subject")
  };
  let public = list(std::iter::once(leaf(&format!("calcit.core/{}", api.public))).chain(raw[1..].iter().cloned()));
  let value = fresh(used);
  let (some_tag, none_tag) = if matches!(api.family, Family::Result) {
    (":ok", ":err")
  } else {
    (":some", ":none")
  };
  let mut pattern = vec![leaf(some_tag), value.clone()];
  let payload = match api.family {
    Family::Map | Family::Set => {
      let mut fields = vec![value.clone()];
      let count = if matches!(api.family, Family::Map) { 3 } else { 2 };
      for _ in 1..count {
        let field = fresh(used);
        pattern.push(field.clone());
        fields.push(field);
      }
      list(std::iter::once(leaf("[]")).chain(fields))
    }
    _ => value,
  };
  let destructured = matches!(api.family, Family::Map | Family::Set);
  let (some, none) = match consumer {
    Consumer::Nil if destructured => (leaf("false"), leaf("true")),
    Consumer::NonNil if destructured => (leaf("true"), leaf("false")),
    Consumer::Nil => (list([leaf("calcit.core/nil?"), payload]), leaf("true")),
    Consumer::NonNil => (list([leaf("calcit.core/non-nil?"), payload]), leaf("false")),
    Consumer::Either => (payload, leaf("nil")),
  };
  let mut empty_pattern = vec![leaf(none_tag)];
  if matches!(api.family, Family::Result) {
    empty_pattern.push(fresh(used));
  }
  let matched = list([
    leaf("match"),
    public,
    list([Cirru::List(pattern), some]),
    list([Cirru::List(empty_pattern), none]),
  ]);
  if matches!(consumer, Consumer::Either) {
    // Preserve each fallback syntax node once, including closure identity.
    list(
      [leaf("calcit.core/either"), matched]
        .into_iter()
        .chain(rewritten[2..].iter().cloned()),
    )
  } else {
    matched
  }
}
