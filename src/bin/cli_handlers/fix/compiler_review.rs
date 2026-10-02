use super::*;
use calcit::calcit::CalcitErr;

pub(super) struct BoundaryReview {
  pub warnings: Vec<LocatedWarning>,
  pub suggestions: Vec<FixSuggestion>,
  pub diagnostics: Vec<Value>,
}

pub(super) fn is_contradictory_proof_warning(warning: &LocatedWarning) -> bool {
  if !warning.code().is_some_and(|code| code.ends_with("_MISMATCH")) {
    return false;
  }
  let owner = warning.location();
  // Macro implementation contracts describe syntax production. The proof
  // pass checks the expanded runtime expression in its caller's scope.
  !matches!(
    program::lookup_def_schema(&owner.ns, &owner.def).as_ref(),
    CalcitTypeAnnotation::Macro(_)
  )
}

/// Preserve strict compiler rejection and expose only source-owned review targets.
/// Preprocessing stops at the first error per definition; this is not an exhaustive scan.
pub(super) fn compile_boundary_review(
  snapshot: &Snapshot,
  snapshot_file: &str,
  definitions: &[(String, String)],
  rule: &'static str,
  workflow: bool,
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
    let result = if workflow
      || matches!(
        rule,
        ASSERT_TYPE_PROOF_RULE | CONCRETE_RETURN_PROOF_RULE | CALLABLE_CONTRACT_PROOF_RULE | NOMINAL_WRITE_PROOF_RULE
      ) {
      runner::preprocess::with_assertion_proof(|| {
        // Reprocess the selected source even if another definition compiled it
        // earlier. Cached local annotations are not pre-assertion evidence.
        runner::preprocess::trace_definition_source_expressions(namespace, definition, &warnings, &CallStackList::default()).map(|_| ())
      })
    } else {
      runner::preprocess::ensure_ns_def_compiled(namespace, definition, &warnings, &CallStackList::default()).map(|_| ())
    };
    if workflow {
      for warning in warnings.borrow().iter().filter(|warning| is_contradictory_proof_warning(warning)) {
        let mut diagnostic = warning.as_json();
        diagnostic["severity"] = serde_json::json!("error");
        diagnostic["phase"] = serde_json::json!("preprocess");
        diagnostic["definition"] = serde_json::json!(format!("{}/{}", warning.location().ns, warning.location().def));
        if !diagnostics.contains(&diagnostic) {
          diagnostics.push(diagnostic);
        }
      }
    }
    if !workflow
      && matches!(
        rule,
        ASSERT_TYPE_PROOF_RULE | CONCRETE_RETURN_PROOF_RULE | CALLABLE_CONTRACT_PROOF_RULE | NOMINAL_WRITE_PROOF_RULE
      )
      && let Some(warning) = warnings.borrow().iter().find(|warning| is_contradictory_proof_warning(warning))
    {
      // A contradictory implementation cannot lend its declared return type
      // to an assertion, even when ordinary checking reports it as a warning.
      return Err(format!("{rule} cannot borrow a contradictory producer contract: {}", warning));
    }
    if let Err(error) = result {
      // One compiler pass serves the combined workflow; keep existing rule identifiers.
      let rule = if workflow {
        match error.code() {
          Some(UNSAFE_COERCE_BOUNDARY_DIAGNOSTIC) => UNSAFE_COERCE_BOUNDARY_RULE,
          Some(ASSERT_TYPE_PROOF_DIAGNOSTIC | "E_ASSERT_TYPE_MISMATCH") => ASSERT_TYPE_PROOF_RULE,
          Some(CONCRETE_RETURN_PROOF_DIAGNOSTIC) => CONCRETE_RETURN_PROOF_RULE,
          Some("E_CALL_ARGUMENT_UNPROVEN") if error.hint.as_deref().is_some_and(|hint| hint.starts_with("Nominal field")) => {
            NOMINAL_WRITE_PROOF_RULE
          }
          Some("E_CALL_ARGUMENT_UNPROVEN") => CALLABLE_CONTRACT_PROOF_RULE,
          _ => {
            let diagnostic = boundary_diagnostic(&error, &format!("{namespace}/{definition}"));
            if !diagnostics.contains(&diagnostic) {
              diagnostics.push(diagnostic);
            }
            continue;
          }
        }
      } else {
        rule
      };
      let diagnostic_code = match (rule, error.code()) {
        (UNSAFE_COERCE_BOUNDARY_RULE, Some(UNSAFE_COERCE_BOUNDARY_DIAGNOSTIC)) => UNSAFE_COERCE_BOUNDARY_DIAGNOSTIC,
        (ASSERT_TYPE_PROOF_RULE, Some(ASSERT_TYPE_PROOF_DIAGNOSTIC)) => ASSERT_TYPE_PROOF_DIAGNOSTIC,
        (ASSERT_TYPE_PROOF_RULE, Some("E_ASSERT_TYPE_MISMATCH")) => "E_ASSERT_TYPE_MISMATCH",
        (ASSERT_TYPE_PROOF_RULE, Some("E_FN_RETURN_UNPROVEN")) => "E_FN_RETURN_UNPROVEN",
        (CONCRETE_RETURN_PROOF_RULE, Some(CONCRETE_RETURN_PROOF_DIAGNOSTIC)) => CONCRETE_RETURN_PROOF_DIAGNOSTIC,
        (
          ASSERT_TYPE_PROOF_RULE | CONCRETE_RETURN_PROOF_RULE | CALLABLE_CONTRACT_PROOF_RULE | NOMINAL_WRITE_PROOF_RULE,
          Some("E_CALL_ARGUMENT_UNPROVEN"),
        ) => "E_CALL_ARGUMENT_UNPROVEN",
        _ => return Err(format!("Failed to preprocess fix target {namespace}/{definition}: {error}")),
      };
      if workflow
        && error.location.as_ref().is_none_or(|location| {
          !definitions
            .iter()
            .any(|(ns, def)| ns == location.ns.as_ref() && def == location.def.as_ref())
        })
      {
        // Preserve an unlocated/dependency error without inventing a writable source path.
        let owner = error.location.as_ref().map_or_else(
          || format!("{namespace}/{definition}"),
          |location| format!("{}/{}", location.ns, location.def),
        );
        let diagnostic = boundary_diagnostic(&error, &owner);
        if !diagnostics.contains(&diagnostic) {
          diagnostics.push(diagnostic);
        }
        continue;
      }
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
      let message = if rule == NOMINAL_WRITE_PROOF_RULE {
        format!(
          "{}; review the declared nominal field and assigned value evidence at this source owner. Choose a checked boundary before the write; do not widen the field, replace the Struct with a Map, or insert an unsafe cast. Only an independently checked producer may supply missing metadata through synthesize-schema-v1. The shared compiler may report an earlier argument obligation first; only the first error per definition is reported, and tests/examples are not scanned. No schema, permission or code is changed.",
          error.msg
        )
      } else if rule == CALLABLE_CONTRACT_PROOF_RULE {
        format!(
          "{}; review the callable's argument, rest, return and feature contract at this source owner. Bare Fn/DynFn storage is allowed but cannot prove a concrete invocation contract. Only an independently checked implementation may supply missing metadata through synthesize-schema-v1; an unknown external callback cannot be assigned a guessed signature. The shared compiler may report an earlier non-callable argument obligation first; only the first error per definition is reported, and tests/examples are not scanned. No schema, permission or code is changed.",
          error.msg
        )
      } else if rule == CONCRETE_RETURN_PROOF_RULE {
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
