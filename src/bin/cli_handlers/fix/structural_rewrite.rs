//! Explicitly reviewed syntactic rewrites through the existing fix transaction.

use super::super::structural_pattern::{instantiate_reviewed_template, match_structure};
use super::*;

pub(super) fn handle_pattern_rewrite(
  options: &FixCommand,
  compiled_snapshot: &Snapshot,
  project_namespaces: &HashSet<String>,
  snapshot_file: &str,
) -> Result<(), String> {
  StructuredOutputFormat::parse(&options.format, "fix")?;
  let (Some(pattern), Some(replacement)) = (&options.pattern, &options.replacement) else {
    return Err("Structural rewrite requires both --pattern and --replace.".to_owned());
  };
  if options.rule.is_some()
    || options.preset.is_some()
    || options.workflow.is_some()
    || options.verify
    || options.replacement_name.is_some()
  {
    return Err("--pattern/--replace conflicts with --rule, --preset, --workflow, --verify and --to.".to_owned());
  }
  if options.apply && options.dry_run {
    return Err("--apply conflicts with --dry-run.".to_owned());
  }
  if options.apply && options.expect_revision.is_none() {
    return Err("Reviewed structural rewrite --apply requires --expect-revision from the reviewed preview.".to_owned());
  }
  if options.definition.is_some() && options.ns.is_none() {
    return Err("--def requires an exact --ns scope.".to_owned());
  }
  let pattern = parse_pattern(pattern)?;
  let replacement = parse_pattern(replacement)?;
  // Validate template bindings even when the source contains no matches.
  let bindings = match_structure(&pattern, &pattern, false, true).expect("a pattern matches its own structure");
  instantiate_reviewed_template(&pattern, &replacement, &bindings)?;
  let _target_scope = FixTargetScope::new(options.ns.is_none());
  let snapshot = load_snapshot(snapshot_file)?;
  let selected = select_definitions(options, &snapshot, project_namespaces)?;
  let content = fs::read_to_string(snapshot_file).map_err(|error| format!("Failed to read {snapshot_file}: {error}"))?;
  let revision = snapshot_content_revision(&content);
  if options.expect_revision.as_deref().is_some_and(|expected| expected != revision) {
    return Err(format!(
      "Snapshot revision mismatch: current revision is '{revision}'. Re-run the preview; no changes were written."
    ));
  }

  if std::env::var("CALCIT_FIX_VALIDATE_ONLY").as_deref() == Ok("1") {
    let warnings = compile_rewrite_scope(&snapshot, &selected, options.include_attached)?;
    println!(
      "{}",
      serde_json::to_string(&warning_identities(&warnings)).map_err(|error| error.to_string())?
    );
    return Ok(());
  }

  let mut suggestions = Vec::new();
  let mut operations = Vec::new();
  for (namespace, definition) in &selected {
    let entry = &snapshot.files[namespace].defs[definition];
    let owner = format!("{namespace}/{definition}");
    let rewritten = rewrite_region(&entry.code, &pattern, &replacement, snapshot_file, &owner, "code", &mut suggestions)?;
    if rewritten != entry.code {
      operations.push(vec![
        "edit".to_owned(),
        "def".to_owned(),
        owner.clone(),
        "--code".to_owned(),
        format_quoted_nodes(&[rewritten])?,
        "--overwrite".to_owned(),
      ]);
    }
    if options.include_attached {
      for test in &entry.tests {
        let rewritten = rewrite_region(
          &test.code,
          &pattern,
          &replacement,
          snapshot_file,
          &owner,
          &format!("tests.{}", test.name),
          &mut suggestions,
        )?;
        if rewritten != test.code {
          let mut tags = test.tags.iter().map(|tag| tag.ref_str()).collect::<Vec<_>>();
          tags.sort_unstable();
          let mut args = vec![
            "edit".to_owned(),
            "add-test".to_owned(),
            owner.clone(),
            test.name.clone(),
            "--code".to_owned(),
            format_quoted_nodes(&[rewritten])?,
            "--overwrite".to_owned(),
          ];
          if !tags.is_empty() {
            args.extend(["--tags".to_owned(), tags.join(",")]);
          }
          operations.push(args);
        }
      }
      let mut examples = Vec::new();
      for (index, example) in entry.examples.iter().enumerate() {
        examples.push(rewrite_region(
          example,
          &pattern,
          &replacement,
          snapshot_file,
          &owner,
          &format!("examples.{index}"),
          &mut suggestions,
        )?);
      }
      if examples != entry.examples {
        operations.push(vec![
          "edit".to_owned(),
          "examples".to_owned(),
          owner,
          "--code".to_owned(),
          format_quoted_nodes(&examples)?,
        ]);
      }
    }
  }

  let transaction = if options.apply && !operations.is_empty() {
    guard_git_worktree(snapshot_file, options.allow_dirty, options.allow_no_vcs)?;
    let mut arguments = fix_scope_args(options);
    arguments.extend([
      "--pattern".to_owned(),
      options.pattern.clone().unwrap(),
      "--replace".to_owned(),
      options.replacement.clone().unwrap(),
    ]);
    run_staged_fix_transaction(
      Path::new(snapshot_file),
      &operations,
      Some(&revision),
      false,
      compiled_snapshot.active_entry_name(),
      &arguments,
      // A reviewed repair may start from invalid legacy source. Require a clean
      // strict staged result instead of demanding proof of the old expression.
      Some(&[]),
    )?
  } else {
    super::super::edit::StagedFixReport {
      changed: false,
      original_revision: revision.clone(),
      new_revision: revision.clone(),
    }
  };
  let report = FixReport {
    schema_version: 1,
    command: "fix",
    revision: &transaction.original_revision,
    data: FixReportData {
      mode: if options.apply { "apply" } else { "preview" },
      filters: FixFilters {
        namespace: options.ns.as_deref(),
        definition: options.definition.as_deref(),
        replacement_name: None,
        rule_id: "pattern",
        preset_id: None,
        expanded_rule_ids: Vec::new(),
        expanded_rules: Vec::new(),
        source_coverage: Some(FixSourceCoverage {
          scanned_regions: if options.include_attached {
            &["code", "tests", "examples"]
          } else {
            &["code"]
          },
          manual_review_regions: if options.include_attached { &[] } else { &["tests", "examples"] },
        }),
      },
      changed: transaction.changed,
      new_revision: &transaction.new_revision,
      validation: FixValidation {
        status: if transaction.changed { "passed" } else { "not-run" },
        staged_scope_preprocess: transaction.changed,
        checked_operations: if transaction.changed { operations.len() } else { 0 },
      },
      suggestions: &suggestions,
      workflow: None,
    },
    diagnostics: Vec::new(),
    next: Vec::new(),
  };
  emit_fix_report(&report, &options.format)
}

fn parse_pattern(code: &str) -> Result<Cirru, String> {
  let mut nodes = cirru_parser::parse(code).map_err(|error| format!("Invalid Cirru structural pattern/template: {error}"))?;
  if nodes.len() != 1 {
    return Err("Structural pattern/template must contain exactly one Cirru expression.".to_owned());
  }
  Ok(nodes.remove(0))
}

fn rewrite_region(
  source: &Cirru,
  pattern: &Cirru,
  replacement: &Cirru,
  snapshot_file: &str,
  owner: &str,
  region: &str,
  suggestions: &mut Vec<FixSuggestion>,
) -> Result<Cirru, String> {
  fn visit(
    source: &Cirru,
    pattern: &Cirru,
    replacement: &Cirru,
    path: &mut Vec<usize>,
    changes: &mut Vec<(Vec<usize>, Cirru, Cirru)>,
  ) -> Result<Cirru, String> {
    if let Cirru::List(items) = source
      && matches!(items.first(), Some(Cirru::Leaf(head)) if matches!(head.rsplit('/').next(), Some("quote" | "quasiquote")))
    {
      return Ok(source.clone());
    }
    let bindings = match_structure(source, pattern, false, true);
    let rewritten = match source {
      Cirru::Leaf(_) => source.clone(),
      Cirru::List(items) => {
        let mut result = Vec::new();
        for (index, child) in items.iter().enumerate() {
          path.push(index);
          result.push(visit(child, pattern, replacement, path, changes)?);
          path.pop();
        }
        Cirru::List(result)
      }
    };
    if bindings.is_some() {
      // Match the original shape, then transport rewritten children. This
      // composes nested original matches once without matching generated code.
      let Some(bindings) = match_structure(&rewritten, pattern, false, true) else {
        return Err(format!(
          "Nested rewrites change the matched structure at {} (literal parts or repeated-variable equality no longer match); split and review the transactions.",
          format_path(path)
        ));
      };
      let result = instantiate_reviewed_template(pattern, replacement, &bindings)?;
      if result != rewritten {
        changes.push((path.clone(), source.clone(), result.clone()));
      }
      Ok(result)
    } else {
      Ok(rewritten)
    }
  }
  let mut changes = Vec::new();
  let rewritten = visit(source, pattern, replacement, &mut Vec::new(), &mut changes)?;
  for (path, original, replacement) in changes {
    suggestions.push(FixSuggestion {
      rule_id: "pattern",
      diagnostic_code: "",
      semantic_layer: "source",
      source_file: snapshot_file.to_owned(),
      definition: owner.to_owned(),
      path: if path.is_empty() { region.to_owned() } else { format!("{region}{}", format_path(&path)) },
      fingerprint: node_fingerprint(&original),
      origin_chain: Vec::new(),
      original: quoted_json(&original),
      replacement: Some(quoted_json(&replacement)),
      applicability: "requires-review",
      message: "User-selected syntactic rewrite; review binding, evaluation order and failure behavior. Strict validation does not prove semantic equivalence.".to_owned(),
      target_path: path,
      operation: None,
    });
  }
  Ok(rewritten)
}

fn compile_rewrite_scope(
  snapshot: &Snapshot,
  selected: &[(String, String)],
  include_attached: bool,
) -> Result<Vec<LocatedWarning>, String> {
  let _output_guard = crate::ProgramOutputGuard::new(true, false);
  let mut warnings = compile_selected_definitions(selected)?;
  if include_attached {
    for (namespace, definition) in selected {
      let entry = &snapshot.files[namespace].defs[definition];
      for (index, source) in entry.tests.iter().map(|test| &test.code).chain(entry.examples.iter()).enumerate() {
        let wrapper = Cirru::List(vec![Cirru::leaf("fn"), Cirru::List(Vec::new()), source.clone()]);
        let synthetic = format!("&calcit:reviewed-rewrite:{definition}:{index}");
        let parsed = code_to_calcit(&wrapper, namespace, &synthetic, vec![]).map_err(|error| error.to_string())?;
        let attached_warnings = RefCell::new(Vec::new());
        runner::preprocess::trace_source_expressions(&parsed, namespace, &synthetic, &attached_warnings, &CallStackList::default())
          .map_err(|failure| format!("Failed to check attached source {namespace}/{definition} #{index}: {failure}"))?;
        warnings.extend(attached_warnings.into_inner());
      }
    }
  }
  Ok(warnings)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn structural_region_paths_label_root_and_nested_code_matches() {
    for (source, expected) in [("legacy 1", "code"), ("wrapper (legacy 1)", "code@1")] {
      let mut suggestions = Vec::new();
      rewrite_region(
        &parse_pattern(source).unwrap(),
        &parse_pattern("legacy ?n").unwrap(),
        &parse_pattern("preferred ?n").unwrap(),
        "calcit.cirru",
        "app.main/main!",
        "code",
        &mut suggestions,
      )
      .unwrap();
      assert_eq!(suggestions.len(), 1);
      assert_eq!(suggestions[0].path, expected);
    }
  }

  #[test]
  fn structural_nested_literal_changes_report_the_failed_path() {
    let error = rewrite_region(
      &parse_pattern("wrapper (a (a (a 1)))").unwrap(),
      &parse_pattern("a (a ?x)").unwrap(),
      &parse_pattern("b ?x").unwrap(),
      "calcit.cirru",
      "app.main/main!",
      "code",
      &mut Vec::new(),
    )
    .unwrap_err();
    assert!(error.contains("matched structure at @1"), "{error}");
    assert!(error.contains("literal parts or repeated-variable equality"), "{error}");
  }
}
