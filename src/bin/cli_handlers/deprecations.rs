//! Deprecated CLI aliases and the normalization of their replacement forms.
//!
//! A non-patch release ships the replacement entry together with a one-line
//! migration hint on stderr; the old entry keeps its stdout contract until the
//! next non-patch release removes it (see `docs/run/upgrade.md`).

use calcit::cli_args::{
  AnalyzeCommand, AnalyzeSubcommand, CalcitCommand, CheckTypesCommand, DeprecatedCommand, DynamicMethodsCommand, QuerySubcommand,
  ToplevelCalcit, TreeSubcommand, WeakTypesCommand,
};

/// View kinds accepted by `analyze weak-types --only` in place of the retired
/// standalone analyzers.
const VIEW_COVERAGE: &str = "coverage";
const VIEW_DYNAMIC_METHOD: &str = "dynamic-method";
const VIEW_DEPRECATED_CALL: &str = "deprecated-call";

/// Returns the one-line migration hint for a deprecated command, if any.
pub fn deprecated_command_hint(cli_args: &ToplevelCalcit) -> Option<String> {
  let (old, new) = match cli_args.subcommand.as_ref()? {
    CalcitCommand::Analyze(cmd) => match &cmd.subcommand {
      AnalyzeSubcommand::CheckTypes(_) => (
        "analyze check-types",
        "`calcit analyze weak-types --only coverage` (level filter: `--coverage-level none,partial,full`)",
      ),
      AnalyzeSubcommand::DynamicMethods(_) => ("analyze dynamic-methods", "`calcit analyze weak-types --only dynamic-method`"),
      AnalyzeSubcommand::Deprecated(_) => ("analyze deprecated", "`calcit analyze weak-types --only deprecated-call`"),
      AnalyzeSubcommand::CheckPublic(_) => (
        "analyze check-public",
        "`calcit --check-only --ns <ns> [--deps] [--summary-only] [--format edn|json]`",
      ),
      AnalyzeSubcommand::Quality(_) => (
        "analyze quality",
        "`calcit --check-only` for correctness and `calcit analyze weak-types [--only coverage|deprecated-call]` to locate debt",
      ),
      _ => return None,
    },
    CalcitCommand::Query(cmd) => match &cmd.subcommand {
      QuerySubcommand::Pkg(_) => ("query pkg", "`calcit query config`, which prints the package name"),
      QuerySubcommand::Modules(_) => ("query modules", "`calcit config modules`"),
      _ => return None,
    },
    CalcitCommand::Tree(cmd) => match &cmd.subcommand {
      TreeSubcommand::BatchDelete(_) => (
        "tree batch-delete",
        "`calcit edit transaction` with one `tree delete` operation per path, highest index first",
      ),
      _ => return None,
    },
    _ => return None,
  };
  Some(format!(
    "[Deprecated] `calcit {old}` will be removed in the next non-patch release; use {new}. See docs/run/upgrade.md."
  ))
}

/// Returns the one-line migration hint for a deprecated operation staged by `edit transaction`.
pub fn deprecated_transaction_operation_hint(index: usize, group: &str, subcommand: &str) -> Option<String> {
  match (group, subcommand) {
    ("tree", "batch-delete") => Some(format!(
      "[Deprecated] transaction operation {} uses `tree batch-delete`, which will be removed in the next non-patch release; use one `tree delete` operation per path, highest index first. See docs/run/upgrade.md.",
      index + 1
    )),
    _ => None,
  }
}

fn view_kind(only: Option<&str>) -> Result<Option<&'static str>, String> {
  let Some(raw) = only else {
    return Ok(None);
  };
  let kinds = raw.split(',').map(str::trim).filter(|kind| !kind.is_empty()).collect::<Vec<_>>();
  let views = [VIEW_COVERAGE, VIEW_DYNAMIC_METHOD, VIEW_DEPRECATED_CALL];
  let selected = views.iter().copied().filter(|view| kinds.contains(view)).collect::<Vec<_>>();
  match (selected.as_slice(), kinds.len()) {
    ([], _) => Ok(None),
    ([view], 1) => Ok(Some(view)),
    _ => Err(format!(
      "`analyze weak-types --only` accepts exactly one view ({}) or a list of match kinds, not a mix: `{raw}`.",
      views.join(", ")
    )),
  }
}

fn reject_options(view: &str, rejected: &[(&str, bool)]) -> Result<(), String> {
  for (flag, present) in rejected {
    if *present {
      return Err(format!("`analyze weak-types --only {view}` does not support `{flag}`."));
    }
  }
  Ok(())
}

/// Rewrites `analyze weak-types --only <view>` to the analyzer that owns that
/// view, so the replacement and the deprecated alias share one implementation
/// and produce identical output.
pub fn normalize_weak_types_view(cli_args: ToplevelCalcit) -> Result<ToplevelCalcit, String> {
  let Some(CalcitCommand::Analyze(AnalyzeCommand {
    subcommand: AnalyzeSubcommand::WeakTypes(options),
  })) = &cli_args.subcommand
  else {
    return Ok(cli_args);
  };
  let view = view_kind(options.only.as_deref())?;
  if view.is_none() {
    if options.coverage_level.is_some() {
      return Err("`--coverage-level` requires `analyze weak-types --only coverage`.".to_owned());
    }
    return Ok(cli_args);
  }
  let options: &WeakTypesCommand = options;
  let evidence_flags = [
    ("--intent", options.intent.is_some()),
    ("--ffi-evidence", options.ffi_evidence),
    ("--schema-evidence", options.schema_evidence),
  ];
  let subcommand = match view {
    Some(VIEW_COVERAGE) => {
      reject_options(VIEW_COVERAGE, &evidence_flags)?;
      AnalyzeSubcommand::CheckTypes(CheckTypesCommand {
        ns: options.ns.clone(),
        ns_prefix: options.ns_prefix.clone(),
        only: options.coverage_level.clone(),
        format: options.format.clone(),
        deps: options.deps,
        summary_only: options.summary_only,
        incremental: options.incremental,
      })
    }
    Some(VIEW_DYNAMIC_METHOD) => {
      reject_options(VIEW_DYNAMIC_METHOD, &evidence_flags)?;
      reject_options(
        VIEW_DYNAMIC_METHOD,
        &[
          ("--ns", options.ns.is_some()),
          ("--ns-prefix", options.ns_prefix.is_some()),
          ("--coverage-level", options.coverage_level.is_some()),
        ],
      )?;
      AnalyzeSubcommand::DynamicMethods(DynamicMethodsCommand {
        format: options.format.clone(),
        deps: options.deps,
        summary_only: options.summary_only,
        incremental: options.incremental,
      })
    }
    Some(VIEW_DEPRECATED_CALL) => {
      reject_options(VIEW_DEPRECATED_CALL, &evidence_flags)?;
      reject_options(
        VIEW_DEPRECATED_CALL,
        &[
          ("--incremental", options.incremental),
          ("--coverage-level", options.coverage_level.is_some()),
        ],
      )?;
      AnalyzeSubcommand::Deprecated(DeprecatedCommand {
        ns: options.ns.clone(),
        ns_prefix: options.ns_prefix.clone(),
        format: options.format.clone(),
        deps: options.deps,
        summary_only: options.summary_only,
      })
    }
    _ => unreachable!("view kind was validated"),
  };
  Ok(ToplevelCalcit {
    subcommand: Some(CalcitCommand::Analyze(AnalyzeCommand { subcommand })),
    ..cli_args
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use argh::FromArgs;

  fn parse(args: &[&str]) -> ToplevelCalcit {
    ToplevelCalcit::from_args(&["calcit"], args).expect("arguments should parse")
  }

  fn analyze(cli_args: &ToplevelCalcit) -> &AnalyzeSubcommand {
    match &cli_args.subcommand {
      Some(CalcitCommand::Analyze(cmd)) => &cmd.subcommand,
      other => panic!("expected analyze command, got {other:?}"),
    }
  }

  #[test]
  fn weak_types_views_share_the_legacy_analyzers() {
    let coverage = normalize_weak_types_view(parse(&[
      "analyze",
      "weak-types",
      "--only",
      "coverage",
      "--coverage-level",
      "none,partial",
      "--ns",
      "app.main",
      "--summary-only",
      "--format",
      "json",
    ]))
    .expect("coverage view");
    let legacy = parse(&[
      "analyze",
      "check-types",
      "--only",
      "none,partial",
      "--ns",
      "app.main",
      "--summary-only",
      "--format",
      "json",
    ]);
    assert_eq!(analyze(&coverage), analyze(&legacy));

    let dynamic =
      normalize_weak_types_view(parse(&["analyze", "weak-types", "--only", "dynamic-method", "--deps"])).expect("dynamic view");
    assert_eq!(analyze(&dynamic), analyze(&parse(&["analyze", "dynamic-methods", "--deps"])));

    let deprecated = normalize_weak_types_view(parse(&[
      "analyze",
      "weak-types",
      "--only",
      "deprecated-call",
      "--ns-prefix",
      "app.",
    ]))
    .expect("view");
    assert_eq!(
      analyze(&deprecated),
      analyze(&parse(&["analyze", "deprecated", "--ns-prefix", "app."]))
    );
  }

  #[test]
  fn weak_types_match_kinds_stay_unchanged() {
    let args = parse(&["analyze", "weak-types", "--only", "code-nil,code-dynamic"]);
    assert_eq!(normalize_weak_types_view(args.clone()).expect("kinds"), args);
  }

  #[test]
  fn weak_types_views_reject_mixed_or_unsupported_options() {
    for args in [
      &["analyze", "weak-types", "--only", "coverage,code-nil"][..],
      &["analyze", "weak-types", "--only", "coverage,deprecated-call"][..],
      &["analyze", "weak-types", "--only", "dynamic-method", "--ns", "app.main"][..],
      &["analyze", "weak-types", "--only", "deprecated-call", "--incremental"][..],
      &["analyze", "weak-types", "--only", "coverage", "--intent", "unresolved"][..],
      &["analyze", "weak-types", "--coverage-level", "none"][..],
    ] {
      assert!(normalize_weak_types_view(parse(args)).is_err(), "{args:?} should be rejected");
    }
  }

  #[test]
  fn deprecated_aliases_name_their_replacement() {
    for (args, replacement) in [
      (&["analyze", "check-types"][..], "weak-types --only coverage"),
      (&["analyze", "dynamic-methods"][..], "weak-types --only dynamic-method"),
      (&["analyze", "deprecated"][..], "weak-types --only deprecated-call"),
      (&["analyze", "check-public", "--ns", "app.lib"][..], "--check-only --ns"),
      (&["analyze", "quality"][..], "--check-only"),
      (&["query", "pkg"][..], "query config"),
      (&["query", "modules"][..], "config modules"),
      (&["tree", "batch-delete", "app.main/f", "--paths", "0"][..], "edit transaction"),
    ] {
      let hint = deprecated_command_hint(&parse(args)).expect("deprecated alias should have a hint");
      assert!(hint.starts_with("[Deprecated]") && hint.contains(replacement), "{hint}");
      assert!(!hint.contains('\n'), "hint should be one line: {hint}");
    }
    assert_eq!(deprecated_command_hint(&parse(&["analyze", "weak-types"])), None);
    assert_eq!(deprecated_command_hint(&parse(&["config", "modules"])), None);
  }
}
