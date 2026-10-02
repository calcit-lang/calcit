use super::*;
use calcit::calcit::CalcitErr;

pub(super) struct BoundaryReview {
  pub warnings: Vec<LocatedWarning>,
  pub suggestions: Vec<FixSuggestion>,
  pub diagnostics: Vec<Value>,
}

/// Preserve strict compiler rejection and expose only source-owned review targets.
/// Preprocessing stops at the first error per definition; this is not an exhaustive scan.
pub(super) fn compile_boundary_review(
  snapshot: &Snapshot,
  snapshot_file: &str,
  definitions: &[(String, String)],
  rule: &'static str,
) -> Result<BoundaryReview, String> {
  if !runner::preprocess::is_strict_types_enabled() {
    return Err(format!(
      "{rule} requires strict types; compatibility mode cannot establish independent proof."
    ));
  }
  let warnings = RefCell::new(Vec::new());
  let mut suggestions = Vec::new();
  let mut diagnostics = Vec::new();
  let mut seen = HashSet::new();
  for (namespace, definition) in definitions {
    let result = if matches!(rule, ASSERT_TYPE_PROOF_RULE | CONCRETE_RETURN_PROOF_RULE) {
      runner::preprocess::with_assertion_proof(|| {
        // Reprocess the selected source even if another definition compiled it
        // earlier. Cached local annotations are not pre-assertion evidence.
        runner::preprocess::trace_definition_source_expressions(namespace, definition, &warnings, &CallStackList::default()).map(|_| ())
      })
    } else {
      runner::preprocess::ensure_ns_def_compiled(namespace, definition, &warnings, &CallStackList::default()).map(|_| ())
    };
    if matches!(rule, ASSERT_TYPE_PROOF_RULE | CONCRETE_RETURN_PROOF_RULE)
      && let Some(warning) = warnings
        .borrow()
        .iter()
        .find(|warning| warning.code() == Some("W_FN_RETURN_TYPE_MISMATCH"))
    {
      // A contradictory implementation cannot lend its declared return type
      // to an assertion, even when ordinary checking reports it as a warning.
      return Err(format!("{rule} cannot borrow a contradictory producer contract: {}", warning));
    }
    if let Err(error) = result {
      let diagnostic_code = match (rule, error.code()) {
        (UNSAFE_COERCE_BOUNDARY_RULE, Some(UNSAFE_COERCE_BOUNDARY_DIAGNOSTIC)) => UNSAFE_COERCE_BOUNDARY_DIAGNOSTIC,
        (ASSERT_TYPE_PROOF_RULE, Some(ASSERT_TYPE_PROOF_DIAGNOSTIC)) => ASSERT_TYPE_PROOF_DIAGNOSTIC,
        (ASSERT_TYPE_PROOF_RULE, Some("E_ASSERT_TYPE_MISMATCH")) => "E_ASSERT_TYPE_MISMATCH",
        (ASSERT_TYPE_PROOF_RULE, Some("E_FN_RETURN_UNPROVEN")) => "E_FN_RETURN_UNPROVEN",
        (CONCRETE_RETURN_PROOF_RULE, Some(CONCRETE_RETURN_PROOF_DIAGNOSTIC)) => CONCRETE_RETURN_PROOF_DIAGNOSTIC,
        _ => return Err(format!("Failed to preprocess fix target {namespace}/{definition}: {error}")),
      };
      let location = error
        .location
        .as_ref()
        .ok_or_else(|| format!("Boundary error has no navigable source location; refusing to guess: {error}"))?;
      if !definitions
        .iter()
        .any(|(ns, def)| ns == location.ns.as_ref() && def == location.def.as_ref())
      {
        return Err(format!(
          "Boundary error is outside the selected project scope; select its source owner explicitly: {error}"
        ));
      }
      let entry = snapshot
        .files
        .get(location.ns.as_ref())
        .and_then(|file| file.defs.get(location.def.as_ref()))
        .ok_or_else(|| format!("Boundary source owner is not in this Snapshot: {error}"))?;
      let path = location.coord.iter().map(|index| usize::from(*index)).collect::<Vec<_>>();
      let original = navigate_to_path(&entry.code, &path)?;
      if !seen.insert((location.ns.clone(), location.def.clone(), path.clone())) {
        continue;
      }
      let mut lexical_path = None;
      for length in (0..=path.len()).rev() {
        let ancestor = navigate_to_path(&entry.code, &path[..length])?;
        if matches!(list_head(&ancestor), Some("fn" | "defn" | "defwasm-export")) {
          lexical_path = Some(if length == 0 {
            "code".to_owned()
          } else {
            format!("code{}", format_path(&path[..length]))
          });
          break;
        }
      }
      let definition_id = format!("{}/{}", location.ns, location.def);
      let diagnostic = boundary_diagnostic(&error, &definition_id);
      let stack = error
        .stack
        .0
        .iter()
        .map(|frame| {
          serde_json::json!({
            "kind": frame.kind.to_string(),
            "definition": format!("{}/{}", frame.ns, frame.def),
          })
        })
        .collect::<Vec<_>>();
      let message = if rule == CONCRETE_RETURN_PROOF_RULE {
        format!(
          "{}; inspect the producer implementation at this source owner, not its wrapper declaration. Choose a checked boundary for open input. Only if the implementation independently proves the missing metadata, review the existing synthesize-schema-v1 candidate. Only the first compiler error per definition is reported; tests/examples are not scanned. No schema, permission or code is changed.",
          error.msg
        )
      } else {
        format!(
          "{}; inspect the compiler location and lexical source context. Review captures, effects and failure paths before choosing a checked decoder or an explicit adapter. Only the first compiler error per definition is reported; tests/examples are not scanned. No permission or code is changed.",
          error.msg
        )
      };
      suggestions.push(FixSuggestion {
        rule_id: rule,
        diagnostic_code,
        semantic_layer: "surface",
        source_file: snapshot_file.to_owned(),
        definition: definition_id,
        path: if path.is_empty() {
          "code".to_owned()
        } else {
          format!("code{}", format_path(&path))
        },
        fingerprint: node_fingerprint(&original),
        origin_chain: vec![serde_json::json!({
          "kind": "compiler-diagnostic",
          "diagnostic": diagnostic,
          "lexical_source_path": lexical_path,
          "stack": stack,
        })],
        original: quoted_json(&original),
        replacement: None,
        applicability: "requires-review",
        message,
        target_path: path,
        operation: None,
      });
      diagnostics.push(diagnostic);
    }
  }
  Ok(BoundaryReview {
    warnings: warnings.into_inner(),
    suggestions,
    diagnostics,
  })
}

pub(super) fn boundary_diagnostic(error: &CalcitErr, definition: &str) -> Value {
  serde_json::json!({
    "code": error.code(),
    "phase": "preprocess",
    "severity": "error",
    "definition": definition,
    "kind": error.kind.to_string().to_lowercase(),
    "message": error.headline(),
    "location": error.location.as_ref().map(|location| serde_json::json!({
      "ns": location.ns.to_string(), "def": location.def.to_string(), "coord": location.coord.to_vec(),
    })),
    "hint": error.hint,
    "provenance": error.provenance,
  })
}
