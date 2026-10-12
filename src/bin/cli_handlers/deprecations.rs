//! Deprecated CLI aliases and the normalization of their replacement forms.
//!
//! A non-patch release ships the replacement entry together with a one-line
//! migration hint on stderr; the old entry keeps its stdout contract until the
//! next non-patch release removes it (see `docs/run/upgrade.md`).

use calcit::cli_args::{
  AnalyzeCommand, AnalyzeSubcommand, CalcitCommand, CallGraphCommand, CallGraphDiffCommand, CheckTypesCommand, CirruSubcommand,
  CountCallsCommand, DeprecatedCommand, DocsCommand, DocsReadLinesCommand, DocsSubcommand, DynamicMethodsCommand, EditSubcommand,
  QuerySubcommand, ToplevelCalcit, TreeAppendChildCommand, TreeInsertAfterCommand, TreeInsertBeforeCommand, TreeInsertChildCommand,
  TreeSubcommand, TreeSwapNextCommand, TreeSwapPrevCommand, WeakTypesCommand,
};

/// View kinds accepted by `analyze weak-types --only` in place of the retired
/// standalone analyzers.
const VIEW_COVERAGE: &str = "coverage";
const VIEW_DYNAMIC_METHOD: &str = "dynamic-method";
const VIEW_DEPRECATED_CALL: &str = "deprecated-call";

/// Old entries without a replacement name this instead of a command.
const NO_REPLACEMENT: &str = "";

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
      AnalyzeSubcommand::CountCalls(_) => (
        "analyze count-calls",
        "`calcit analyze call-graph --counts` (with `--sort count|name` and `--format human|edn|json`)",
      ),
      AnalyzeSubcommand::CallGraphDiff(_) => ("analyze call-graph-diff", "`calcit analyze call-graph --diff <ref>`"),
      AnalyzeSubcommand::JsEscape(_) => ("analyze js-escape", NO_REPLACEMENT),
      AnalyzeSubcommand::JsUnescape(_) => ("analyze js-unescape", NO_REPLACEMENT),
      _ => return None,
    },
    CalcitCommand::Query(cmd) => match &cmd.subcommand {
      QuerySubcommand::Pkg(_) => ("query pkg", "`calcit query config`, which prints the package name"),
      QuerySubcommand::Modules(_) => ("query modules", "`calcit config modules`"),
      QuerySubcommand::HostProcs(_) => ("query host-procs", NO_REPLACEMENT),
      _ => return None,
    },
    CalcitCommand::Tree(cmd) => match &cmd.subcommand {
      TreeSubcommand::BatchDelete(_) => (
        "tree batch-delete",
        "`calcit edit transaction` with one `tree delete` operation per path, highest index first",
      ),
      TreeSubcommand::InsertBefore(_) => ("tree insert-before", "`calcit tree insert --at before`"),
      TreeSubcommand::InsertAfter(_) => ("tree insert-after", "`calcit tree insert --at after`"),
      TreeSubcommand::InsertChild(_) => ("tree insert-child", "`calcit tree insert --at prepend-child`"),
      TreeSubcommand::AppendChild(_) => ("tree append-child", "`calcit tree insert --at append-child`"),
      TreeSubcommand::SwapNext(_) => ("tree swap-next", "`calcit tree swap --with next`"),
      TreeSubcommand::SwapPrev(_) => ("tree swap-prev", "`calcit tree swap --with prev`"),
      _ => return None,
    },
    CalcitCommand::Edit(cmd) => match &cmd.subcommand {
      EditSubcommand::RmExample(_) => ("edit rm-example", REPLACE_RM_EXAMPLE),
      EditSubcommand::NsDoc(_) => ("edit ns-doc", REPLACE_NS_DOC),
      _ => return None,
    },
    CalcitCommand::Docs(cmd) => match &cmd.subcommand {
      DocsSubcommand::ReadLines(_) => ("docs read-lines", "`calcit docs read <file> --start <n> --lines <n>`"),
      _ => return None,
    },
    CalcitCommand::Cirru(cmd) => match &cmd.subcommand {
      CirruSubcommand::ShowGuide(_) => ("cirru show-guide", "`calcit docs read cirru-syntax.md`"),
      _ => return None,
    },
    _ => return None,
  };
  Some(format_hint(&format!("`calcit {old}`"), new))
}

const REPLACE_RM_EXAMPLE: &str = "`calcit edit examples <ns/def>` with the remaining examples (from `query examples`), or `calcit edit ns` with the changed `:examples`";
const REPLACE_NS_DOC: &str = "`calcit edit ns <ns>` with the changed `:doc` of the `:ns` entry (from `query ns <ns> --format cirru`)";

fn format_hint(subject: &str, new: &str) -> String {
  if new == NO_REPLACEMENT {
    format!("[Deprecated] {subject} will be removed in the next non-patch release and has no replacement. See docs/run/upgrade.md.")
  } else {
    format!("[Deprecated] {subject} will be removed in the next non-patch release; use {new}. See docs/run/upgrade.md.")
  }
}

/// Returns the one-line migration hint for the retired type-policy flags.
///
/// Strict diagnostics are the default, so `--strict-types` only restates the
/// default (it still pre-checks the entry) and `--compat-types` is a temporary
/// escape hatch; both flags stay accepted until the next non-patch release.
pub fn deprecated_type_policy_flag_hint(strict_types: bool, compat_types: bool) -> Option<String> {
  match (strict_types, compat_types) {
    (true, false) => Some(format_hint(
      "`--strict-types`",
      "the default strict diagnostics (pair them with `--check-only` to pre-check the entry)",
    )),
    (false, true) => Some(
      "[Deprecated] `--compat-types` will be removed in the next non-patch release and has no replacement; fix the reported type diagnostics instead. See docs/run/upgrade.md."
        .to_owned(),
    ),
    (true, true) => Some(format_hint("`--strict-types` and `--compat-types`", NO_REPLACEMENT)),
    (false, false) => None,
  }
}

/// Returns the one-line migration hint for a deprecated operation staged by `edit transaction`.
pub fn deprecated_transaction_operation_hint(index: usize, group: &str, subcommand: &str) -> Option<String> {
  match (group, subcommand) {
    ("tree", "batch-delete") => Some(format!(
      "[Deprecated] transaction operation {} uses `tree batch-delete`, which will be removed in the next non-patch release; use one `tree delete` operation per path, highest index first. See docs/run/upgrade.md.",
      index + 1
    )),
    ("tree", "insert-before" | "insert-after" | "insert-child" | "append-child" | "swap-next" | "swap-prev") => {
      let replacement = match subcommand {
        "insert-before" => "`tree insert --at before`",
        "insert-after" => "`tree insert --at after`",
        "insert-child" => "`tree insert --at prepend-child`",
        "append-child" => "`tree insert --at append-child`",
        "swap-next" => "`tree swap --with next`",
        _ => "`tree swap --with prev`",
      };
      Some(format!(
        "[Deprecated] transaction operation {} uses `tree {subcommand}`, which will be removed in the next non-patch release; use {replacement}. See docs/run/upgrade.md.",
        index + 1
      ))
    }
    ("edit", "rm-example") => Some(format!(
      "[Deprecated] transaction operation {} uses `edit rm-example`, which will be removed in the next non-patch release; use an `edit examples` operation with the remaining examples. See docs/run/upgrade.md.",
      index + 1
    )),
    ("edit", "ns-doc") => Some(format!(
      "[Deprecated] transaction operation {} uses `edit ns-doc`, which will be removed in the next non-patch release; run `calcit edit ns <ns>` with the changed `:ns` doc as its own guarded step. See docs/run/upgrade.md.",
      index + 1
    )),
    _ => None,
  }
}

/// Rewrites `tree insert --at <position>` and `tree swap --with <next|prev>` to the
/// leaf handlers they replace, so both spellings share one implementation.
pub fn normalize_tree_subcommand(subcommand: &TreeSubcommand) -> Result<TreeSubcommand, String> {
  match subcommand {
    TreeSubcommand::Insert(o) => {
      let (target, path, file, code, input_format, expect, depth) = (
        o.target.clone(),
        o.path.clone(),
        o.file.clone(),
        o.code.clone(),
        o.input_format,
        o.expect.clone(),
        o.depth,
      );
      Ok(match o.at.as_str() {
        "before" => TreeSubcommand::InsertBefore(TreeInsertBeforeCommand {
          target,
          path,
          file,
          code,
          input_format,
          expect,
          depth,
        }),
        "after" => TreeSubcommand::InsertAfter(TreeInsertAfterCommand {
          target,
          path,
          file,
          code,
          input_format,
          expect,
          depth,
        }),
        "prepend-child" => TreeSubcommand::InsertChild(TreeInsertChildCommand {
          target,
          path,
          file,
          code,
          input_format,
          expect,
          depth,
        }),
        "append-child" => TreeSubcommand::AppendChild(TreeAppendChildCommand {
          target,
          path,
          file,
          code,
          input_format,
          expect,
          depth,
        }),
        other => {
          return Err(format!(
            "Unsupported `tree insert --at {other}`. Use: before, after, prepend-child, append-child."
          ));
        }
      })
    }
    TreeSubcommand::Swap(o) => {
      let (target, path, depth) = (o.target.clone(), o.path.clone(), o.depth);
      Ok(match o.with.as_str() {
        "next" => TreeSubcommand::SwapNext(TreeSwapNextCommand { target, path, depth }),
        "prev" => TreeSubcommand::SwapPrev(TreeSwapPrevCommand { target, path, depth }),
        other => return Err(format!("Unsupported `tree swap --with {other}`. Use: next, prev.")),
      })
    }
    other => Ok(other.clone()),
  }
}

/// Rewrites every consolidated entry to the handler that owns it, so a
/// replacement and its deprecated alias share one implementation.
pub fn normalize_consolidated_views(cli_args: ToplevelCalcit) -> Result<ToplevelCalcit, String> {
  let cli_args = normalize_weak_types_view(cli_args)?;
  let cli_args = normalize_call_graph_view(cli_args)?;
  normalize_docs_read_lines(cli_args)
}

/// Rewrites `analyze call-graph --counts` and `--diff <ref>` to the
/// count-calls and call-graph-diff analyzers.
fn normalize_call_graph_view(cli_args: ToplevelCalcit) -> Result<ToplevelCalcit, String> {
  let Some(CalcitCommand::Analyze(AnalyzeCommand {
    subcommand: AnalyzeSubcommand::CallGraph(options),
  })) = &cli_args.subcommand
  else {
    return Ok(cli_args);
  };
  let options: &CallGraphCommand = options;
  let subcommand = match (options.counts, &options.diff) {
    (true, Some(_)) => return Err("`analyze call-graph` accepts `--counts` or `--diff <ref>`, not both.".to_owned()),
    (false, None) => {
      if options.sort.is_some() {
        return Err("`--sort` requires `analyze call-graph --counts`.".to_owned());
      }
      return Ok(cli_args);
    }
    (true, None) => {
      reject_flags(
        "analyze call-graph --counts",
        &[("--max-depth", options.max_depth != 0), ("--show-unused", options.show_unused)],
      )?;
      AnalyzeSubcommand::CountCalls(CountCallsCommand {
        root: options.root.clone(),
        ns_prefix: options.ns_prefix.clone(),
        include_core: options.include_core,
        format: options.format.clone(),
        sort: options.sort.clone().unwrap_or_else(|| "count".to_owned()),
      })
    }
    (false, Some(git_ref)) => {
      reject_flags(
        "analyze call-graph --diff",
        &[
          ("--show-unused", options.show_unused),
          ("--format", !matches!(options.format.as_str(), "text" | "tree" | "human")),
          ("--sort", options.sort.is_some()),
        ],
      )?;
      AnalyzeSubcommand::CallGraphDiff(CallGraphDiffCommand {
        git_ref: git_ref.clone(),
        root: options.root.clone(),
        ns_prefix: options.ns_prefix.clone(),
        include_core: options.include_core,
        max_depth: options.max_depth,
      })
    }
  };
  Ok(ToplevelCalcit {
    subcommand: Some(CalcitCommand::Analyze(AnalyzeCommand { subcommand })),
    ..cli_args
  })
}

/// Rewrites `docs read <file> --start <n> --lines <n>` to the line-range reader.
fn normalize_docs_read_lines(cli_args: ToplevelCalcit) -> Result<ToplevelCalcit, String> {
  let Some(CalcitCommand::Docs(DocsCommand {
    subcommand: DocsSubcommand::Read(options),
  })) = &cli_args.subcommand
  else {
    return Ok(cli_args);
  };
  if options.start.is_none() && options.lines.is_none() {
    return Ok(cli_args);
  }
  reject_flags(
    "docs read --start/--lines",
    &[
      ("<headings>", !options.headings.is_empty()),
      ("--no-subheadings", options.no_subheadings),
      ("--full", options.full),
      ("--with-lines", options.with_lines),
    ],
  )?;
  let subcommand = DocsSubcommand::ReadLines(DocsReadLinesCommand {
    filename: options.filename.clone(),
    start: options.start.unwrap_or(0),
    lines: options.lines.unwrap_or(80),
    module: options.module.clone(),
  });
  Ok(ToplevelCalcit {
    subcommand: Some(CalcitCommand::Docs(DocsCommand { subcommand })),
    ..cli_args
  })
}

fn reject_flags(form: &str, rejected: &[(&str, bool)]) -> Result<(), String> {
  for (flag, present) in rejected {
    if *present {
      return Err(format!("`{form}` does not support `{flag}`."));
    }
  }
  Ok(())
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
  reject_flags(&format!("analyze weak-types --only {view}"), rejected)
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
  #[test]
  fn type_policy_flags_report_deprecation_only_when_used() {
    use super::deprecated_type_policy_flag_hint as hint;
    assert!(hint(false, false).is_none());
    assert!(hint(true, false).unwrap().contains("`--strict-types`"));
    assert!(hint(false, true).unwrap().contains("`--compat-types`"));
    assert!(hint(true, true).unwrap().contains("`--strict-types` and `--compat-types`"));
  }

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
      (&["query", "host-procs"][..], "has no replacement"),
      (
        &["tree", "insert-before", "app.main/f", "--path", "0"][..],
        "tree insert --at before",
      ),
      (&["tree", "insert-after", "app.main/f", "--path", "0"][..], "tree insert --at after"),
      (
        &["tree", "insert-child", "app.main/f", "--path", "0"][..],
        "tree insert --at prepend-child",
      ),
      (
        &["tree", "append-child", "app.main/f", "--path", "0"][..],
        "tree insert --at append-child",
      ),
      (&["tree", "swap-next", "app.main/f", "--path", "0"][..], "tree swap --with next"),
      (&["tree", "swap-prev", "app.main/f", "--path", "0"][..], "tree swap --with prev"),
      (&["tree", "batch-delete", "app.main/f", "--paths", "0"][..], "edit transaction"),
      (&["analyze", "count-calls"][..], "call-graph --counts"),
      (&["analyze", "call-graph-diff", "HEAD"][..], "call-graph --diff"),
      (&["analyze", "js-escape", "a?"][..], "has no replacement"),
      (&["analyze", "js-unescape", "a_$q_"][..], "has no replacement"),
      (&["edit", "rm-example", "app.main/f", "0"][..], "edit examples"),
      (&["edit", "ns-doc", "app.main", "doc"][..], "edit ns <ns>"),
      (&["docs", "read-lines", "a.md"][..], "docs read <file> --start"),
      (&["cirru", "show-guide"][..], "docs read cirru-syntax.md"),
    ] {
      let hint = deprecated_command_hint(&parse(args)).expect("deprecated alias should have a hint");
      assert!(hint.starts_with("[Deprecated]") && hint.contains(replacement), "{hint}");
      assert!(!hint.contains('\n'), "hint should be one line: {hint}");
    }
    assert_eq!(deprecated_command_hint(&parse(&["analyze", "weak-types"])), None);
    assert_eq!(deprecated_command_hint(&parse(&["config", "modules"])), None);
    assert_eq!(deprecated_command_hint(&parse(&["analyze", "call-graph", "--counts"])), None);
    assert_eq!(deprecated_command_hint(&parse(&["docs", "read", "a.md", "--lines", "3"])), None);
    assert_eq!(deprecated_command_hint(&parse(&["edit", "ns", "app.main"])), None);
  }

  #[test]
  fn tree_insert_and_swap_share_the_legacy_leaves() {
    let tree = |args: &[&str]| match parse(args).subcommand {
      Some(CalcitCommand::Tree(cmd)) => cmd.subcommand,
      other => panic!("expected tree command, got {other:?}"),
    };
    for (at, old) in [
      ("before", "insert-before"),
      ("after", "insert-after"),
      ("prepend-child", "insert-child"),
      ("append-child", "append-child"),
    ] {
      let new = tree(&[
        "tree",
        "insert",
        "app.main/f",
        "--path",
        "2.1",
        "--at",
        at,
        "--code",
        "quote x",
        "--input-format",
        "cirru",
        "--expect",
        "quote y",
        "--depth",
        "3",
      ]);
      let legacy = tree(&[
        "tree",
        old,
        "app.main/f",
        "--path",
        "2.1",
        "--code",
        "quote x",
        "--input-format",
        "cirru",
        "--expect",
        "quote y",
        "--depth",
        "3",
      ]);
      assert_eq!(normalize_tree_subcommand(&new).expect("insert"), legacy, "--at {at}");
    }
    for (with, old) in [("next", "swap-next"), ("prev", "swap-prev")] {
      let new = tree(&["tree", "swap", "app.main/f", "--path", "2.1", "--with", with]);
      assert_eq!(
        normalize_tree_subcommand(&new).expect("swap"),
        tree(&["tree", old, "app.main/f", "--path", "2.1"])
      );
    }
    assert!(normalize_tree_subcommand(&tree(&["tree", "insert", "a/b", "--path", "0", "--at", "around"])).is_err());
    assert!(normalize_tree_subcommand(&tree(&["tree", "swap", "a/b", "--path", "0", "--with", "up"])).is_err());
    assert_eq!(
      deprecated_command_hint(&parse(&["tree", "insert", "a/b", "--path", "0", "--at", "after"])),
      None
    );
    assert_eq!(
      deprecated_command_hint(&parse(&["tree", "swap", "a/b", "--path", "0", "--with", "next"])),
      None
    );
  }

  #[test]
  fn call_graph_options_share_the_legacy_analyzers() {
    let normalize = |args: &[&str]| normalize_consolidated_views(parse(args)).expect("call-graph option");
    assert_eq!(
      analyze(&normalize(&[
        "analyze",
        "call-graph",
        "--counts",
        "--sort",
        "name",
        "--format",
        "json",
        "--root",
        "app.main/main!",
        "--include-core",
      ])),
      analyze(&parse(&[
        "analyze",
        "count-calls",
        "--sort",
        "name",
        "--format",
        "json",
        "--root",
        "app.main/main!",
        "--include-core",
      ]))
    );
    assert_eq!(
      analyze(&normalize(&["analyze", "call-graph", "--counts"])),
      analyze(&parse(&["analyze", "count-calls"]))
    );
    assert_eq!(
      analyze(&normalize(&[
        "analyze",
        "call-graph",
        "--diff",
        "HEAD~1",
        "--max-depth",
        "3",
        "--ns-prefix",
        "app."
      ])),
      analyze(&parse(&[
        "analyze",
        "call-graph-diff",
        "HEAD~1",
        "--max-depth",
        "3",
        "--ns-prefix",
        "app."
      ]))
    );
    let plain = parse(&["analyze", "call-graph", "--show-unused"]);
    assert_eq!(normalize_consolidated_views(plain.clone()).expect("plain call-graph"), plain);
    for args in [
      &["analyze", "call-graph", "--counts", "--diff", "HEAD"][..],
      &["analyze", "call-graph", "--sort", "name"][..],
      &["analyze", "call-graph", "--counts", "--max-depth", "2"][..],
      &["analyze", "call-graph", "--counts", "--show-unused"][..],
      &["analyze", "call-graph", "--diff", "HEAD", "--format", "json"][..],
      &["analyze", "call-graph", "--diff", "HEAD", "--show-unused"][..],
    ] {
      assert!(normalize_consolidated_views(parse(args)).is_err(), "{args:?} should be rejected");
    }
  }

  #[test]
  fn docs_read_line_range_shares_the_legacy_reader() {
    let docs = |cli_args: &ToplevelCalcit| match &cli_args.subcommand {
      Some(CalcitCommand::Docs(cmd)) => cmd.subcommand.clone(),
      other => panic!("expected docs command, got {other:?}"),
    };
    for (new, old) in [
      (
        &["docs", "read", "a.md", "--start", "4", "--lines", "9", "--module", "respo.calcit"][..],
        &[
          "docs",
          "read-lines",
          "a.md",
          "--start",
          "4",
          "--lines",
          "9",
          "--module",
          "respo.calcit",
        ][..],
      ),
      (
        &["docs", "read", "a.md", "--start", "4"][..],
        &["docs", "read-lines", "a.md", "--start", "4"][..],
      ),
      (
        &["docs", "read", "a.md", "--lines", "9"][..],
        &["docs", "read-lines", "a.md", "--lines", "9"][..],
      ),
    ] {
      let normalized = normalize_consolidated_views(parse(new)).expect("line range");
      assert_eq!(docs(&normalized), docs(&parse(old)));
    }
    let sections = parse(&["docs", "read", "a.md", "Intro"]);
    assert_eq!(normalize_consolidated_views(sections.clone()).expect("sections"), sections);
    for args in [
      &["docs", "read", "a.md", "Intro", "--lines", "3"][..],
      &["docs", "read", "a.md", "--full", "--start", "3"][..],
      &["docs", "read", "a.md", "--with-lines", "--lines", "3"][..],
    ] {
      assert!(normalize_consolidated_views(parse(args)).is_err(), "{args:?} should be rejected");
    }
  }

  #[test]
  fn deprecated_transaction_operations_name_their_replacement() {
    for (group, subcommand, replacement) in [
      ("tree", "batch-delete", "tree delete"),
      ("tree", "insert-before", "tree insert --at before"),
      ("tree", "append-child", "tree insert --at append-child"),
      ("tree", "swap-prev", "tree swap --with prev"),
      ("edit", "rm-example", "edit examples"),
      ("edit", "ns-doc", "edit ns <ns>"),
    ] {
      let hint = deprecated_transaction_operation_hint(1, group, subcommand).expect("deprecated operation should have a hint");
      assert!(
        hint.starts_with("[Deprecated] transaction operation 2") && hint.contains(replacement),
        "{hint}"
      );
    }
    assert_eq!(deprecated_transaction_operation_hint(0, "edit", "examples"), None);
  }
}
