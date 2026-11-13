//! `effect-features-v1` (#2710): add the `:state` / `:io` features a
//! definition's `Fn` schema is missing.
//!
//! The compiler records, for each typed project function, the effects its own
//! body causes and the project definitions it uses. Those features propagate
//! to callers, so the plan solves the graph to a fixpoint and adds every
//! missing feature at once instead of one layer per run.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::*;
use calcit::runner::preprocess::EffectSummary;

pub(super) const EFFECT_FEATURES_RULE: &str = "effect-features-v1";
pub(super) const EFFECT_FEATURES_DIAGNOSTIC: &str = "E_STATE_FEATURE_REQUIRED";
const IO_DIAGNOSTIC: &str = "E_IO_FEATURE_REQUIRED";

type DefKey = (Arc<str>, Arc<str>);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Needs {
  state: bool,
  io: bool,
}

impl Needs {
  fn union(self, other: Needs) -> Needs {
    Needs {
      state: self.state || other.state,
      io: self.io || other.io,
    }
  }

  fn from_features(signature: &CalcitFnTypeAnnotation) -> Needs {
    Needs {
      state: signature.features.iter().any(|tag| tag.ref_str() == "state"),
      io: signature.features.iter().any(|tag| tag.ref_str() == "io"),
    }
  }
}

fn fn_signature(schema: &CalcitTypeAnnotation) -> Option<&CalcitFnTypeAnnotation> {
  match schema {
    CalcitTypeAnnotation::Fn(signature) => Some(signature.as_ref()),
    _ => None,
  }
}

/// Plan schema edits from the summaries recorded while compiling the project.
pub(super) fn plan(
  options: &FixCommand,
  snapshot: &Snapshot,
  snapshot_file: &str,
  summaries: &HashMap<DefKey, EffectSummary>,
) -> Result<Vec<FixSuggestion>, String> {
  // Only editable definitions with a structured Fn schema can take features.
  let mut declared: BTreeMap<DefKey, Needs> = BTreeMap::new();
  for (ns, def) in summaries.keys() {
    let Some(entry) = snapshot.files.get(ns.as_ref()).and_then(|file| file.defs.get(def.as_ref())) else {
      continue;
    };
    if let Some(signature) = fn_signature(entry.schema.as_ref()) {
      declared.insert((ns.clone(), def.clone()), Needs::from_features(signature));
    }
  }

  let mut needed = declared.clone();
  loop {
    let mut changed = false;
    for (key, current) in needed.clone() {
      let summary = &summaries[&key];
      let mut next = current.union(Needs {
        state: summary.state,
        io: summary.io,
      });
      for dep in &summary.deps {
        let dep_needs = match needed.get(dep) {
          Some(needs) => *needs,
          // Library definitions keep the features their schema declares.
          None => fn_signature(program::lookup_def_schema(&dep.0, &dep.1).as_ref())
            .map(Needs::from_features)
            .unwrap_or_default(),
        };
        next = next.union(dep_needs);
      }
      if next != current {
        needed.insert(key, next);
        changed = true;
      }
    }
    if !changed {
      break;
    }
  }

  let mut suggestions = Vec::new();
  for (key, needs) in &needed {
    let (ns, def) = key;
    if options.ns.as_deref().is_some_and(|filter| filter != ns.as_ref())
      || options.definition.as_deref().is_some_and(|filter| filter != def.as_ref())
    {
      continue;
    }
    let before = declared[key];
    let mut added = Vec::new();
    if needs.state && !before.state {
      added.push("state");
    }
    if needs.io && !before.io {
      added.push("io");
    }
    if added.is_empty() {
      continue;
    }
    let entry = &snapshot.files[ns.as_ref()].defs[def.as_ref()];
    let signature = fn_signature(entry.schema.as_ref()).expect("declared holds Fn schemas only");
    let mut updated = signature.clone();
    let mut features = updated.features.as_ref().clone();
    for feature in &added {
      features.insert(cirru_edn::EdnTag::new(*feature));
    }
    updated.features = Arc::new(features);
    let original_node = schema_edn_to_source_node(&calcit::snapshot::schema_annotation_to_edn(entry.schema.as_ref()))?;
    let replacement_node = schema_edn_to_source_node(&calcit::snapshot::schema_annotation_to_edn(&CalcitTypeAnnotation::Fn(
      Arc::new(updated),
    )))?;

    let summary = &summaries[key];
    let mut origin_chain = Vec::new();
    let mut direct = BTreeSet::new();
    if summary.state && added.contains(&"state") {
      direct.insert("state");
    }
    if summary.io && added.contains(&"io") {
      direct.insert("io");
    }
    for feature in &direct {
      origin_chain.push(serde_json::json!({"kind": "direct-effect", "feature": feature}));
    }
    for dep in &summary.deps {
      let dep_needs = needed.get(dep).copied().unwrap_or_else(|| {
        fn_signature(program::lookup_def_schema(&dep.0, &dep.1).as_ref())
          .map(Needs::from_features)
          .unwrap_or_default()
      });
      for (feature, flag) in [("state", dep_needs.state), ("io", dep_needs.io)] {
        if flag && added.contains(&feature) && !direct.contains(feature) {
          origin_chain.push(serde_json::json!({
            "kind": "dependency",
            "definition": format!("{}/{}", dep.0, dep.1),
            "feature": feature,
          }));
        }
      }
    }
    let features_text = added
      .iter()
      .map(|feature| format!("`:{feature}`"))
      .collect::<Vec<_>>()
      .join(" and ");
    suggestions.push(FixSuggestion {
      rule_id: EFFECT_FEATURES_RULE,
      diagnostic_code: if added.contains(&"state") {
        EFFECT_FEATURES_DIAGNOSTIC
      } else {
        IO_DIAGNOSTIC
      },
      semantic_layer: "surface",
      source_file: snapshot_file.to_owned(),
      definition: format!("{ns}/{def}"),
      path: "schema".to_owned(),
      fingerprint: node_fingerprint(&original_node),
      origin_chain,
      original: quoted_json(&original_node),
      replacement: Some(quoted_json(&replacement_node)),
      applicability: "machine-applicable",
      message: format!(
        "Add {features_text} to the Fn schema: the body causes this effect or uses a definition that does. Callers inherit it; the checked types are unchanged."
      ),
      target_path: vec![],
      operation: Some(FixOperation::ReplaceSchema {
        code: format_schema_operation(&replacement_node)?,
      }),
    });
  }
  Ok(suggestions)
}
