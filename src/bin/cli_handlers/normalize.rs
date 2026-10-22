//! Normalization of merged CLI forms, and the migration messages for the
//! entries removed in 0.31.0 (#2033, see `docs/run/upgrade.md`).
//!
//! The few deprecated entries that still have known ecosystem consumers keep a
//! one-line stderr hint until a later non-patch release removes them.

use calcit::cli_args::{
  AnalyzeCommand, AnalyzeSubcommand, CalcitCommand, CallGraphCommand, CallGraphDiffCommand, CheckTypesCommand, CountCallsCommand,
  CursorApplyCommand, CursorSetCommand, CursorShowCommand, CursorSubcommand, DeprecatedCommand, DynamicMethodsCommand, ToplevelCalcit,
  WeakTypesCommand,
};

/// View kinds accepted by `analyze weak-types --only` in place of the retired
/// standalone analyzers.
const VIEW_COVERAGE: &str = "coverage";
const VIEW_DYNAMIC_METHOD: &str = "dynamic-method";
const VIEW_DEPRECATED_CALL: &str = "deprecated-call";
pub(crate) const VIEW_OPTION_NIL_ROUNDTRIP: &str = "option-nil-roundtrip";

/// Removed entries without a replacement name this instead of a command.
const NO_REPLACEMENT: &str = "";

const REPLACE_RM_EXAMPLE: &str = "`calcit edit examples <ns/def>` with the remaining examples (from `query examples`), or `calcit edit ns` with the changed `:examples`";
const REPLACE_NS_DOC: &str = "`calcit edit ns <ns>` with the changed `:doc` of the `:ns` entry (from `query ns <ns> --format cirru`)";

/// Command groups whose leaves were removed in 0.31.0.
const REMOVED_GROUPS: &[&str] = &["analyze", "query", "tree", "cursor", "edit", "docs", "cirru", "config"];

/// `(group, leaf, replacement)` for every CLI entry removed in 0.31.0.
const REMOVED_COMMANDS: &[(&str, &str, &str)] = &[
  ("analyze", "dynamic-methods", "`calcit analyze weak-types --only dynamic-method`"),
  ("analyze", "deprecated", "`calcit analyze weak-types --only deprecated-call`"),
  (
    "analyze",
    "count-calls",
    "`calcit analyze call-graph --counts` (with `--sort count|name` and `--format human|edn|json`)",
  ),
  ("analyze", "call-graph-diff", "`calcit analyze call-graph --diff <ref>`"),
  ("analyze", "js-escape", NO_REPLACEMENT),
  ("analyze", "js-unescape", NO_REPLACEMENT),
  ("query", "pkg", "`calcit query config`, which prints the package name"),
  ("query", "modules", "`calcit config modules`"),
  ("query", "host-procs", NO_REPLACEMENT),
  (
    "tree",
    "batch-delete",
    "`calcit edit transaction` with one `tree delete` operation per path, highest index first",
  ),
  ("tree", "insert-before", "`calcit tree insert --at before`"),
  ("tree", "insert-after", "`calcit tree insert --at after`"),
  ("tree", "insert-child", "`calcit tree insert --at prepend-child`"),
  ("tree", "append-child", "`calcit tree insert --at append-child`"),
  ("tree", "swap-next", "`calcit tree swap --with next`"),
  ("tree", "swap-prev", "`calcit tree swap --with prev`"),
  ("cursor", "parent", "`calcit cursor move parent`"),
  ("cursor", "child", "`calcit cursor move child [index] [--last]`"),
  ("cursor", "next", "`calcit cursor move next [--count n]`"),
  ("cursor", "prev", "`calcit cursor move prev [--count n]`"),
  ("cursor", "back", "`calcit cursor move back [--count n]`"),
  ("cursor", "forward", "`calcit cursor move forward [--count n]`"),
  ("cursor", "backward", "`calcit cursor move backward [--count n]`"),
  ("cursor", "clear-anchor", "`calcit cursor anchor clear`"),
  ("cursor", "region", "`calcit cursor anchor show`"),
  ("cursor", "mark", "`calcit cursor marks set <name>`"),
  ("cursor", "goto", "`calcit cursor marks go <name>`"),
  ("cursor", "rm-mark", "`calcit cursor marks drop <name>`"),
  ("cursor", "copy", "`calcit cursor clipboard copy`"),
  ("cursor", "cut", "`calcit cursor clipboard cut`"),
  ("cursor", "paste", "`calcit cursor clipboard paste [--at <position>]`"),
  ("cursor", "clear-clipboard", "`calcit cursor clipboard clear`"),
  ("cursor", "slurp-next", "`calcit cursor apply slurp-next`"),
  ("cursor", "slurp-prev", "`calcit cursor apply slurp-prev`"),
  ("cursor", "barf-last", "`calcit cursor apply barf-last`"),
  ("cursor", "barf-first", "`calcit cursor apply barf-first`"),
  ("cursor", "duplicate", "`calcit cursor apply duplicate [--at before|after]`"),
  ("edit", "rm-example", REPLACE_RM_EXAMPLE),
  ("edit", "ns-doc", REPLACE_NS_DOC),
  ("docs", "read-lines", "`calcit docs read <file> --start <n> --lines <n>`"),
  ("cirru", "show-guide", "`calcit docs read cirru-syntax.md`"),
  (
    "config",
    "version",
    "`caps version get/set/bump`, which manages `deps.cirru :version`",
  ),
];

/// Commands whose `--json` switch was removed in 0.31.0. `ffi export --json` stays
/// deprecated because the calcit.std FFI inventory workflow still documents it.
const REMOVED_JSON_SWITCH: &[(&str, &str)] = &[("query", "schema"), ("query", "def"), ("tree", "show")];

fn removed_replacement(group: &str, leaf: &str) -> Option<&'static str> {
  REMOVED_COMMANDS
    .iter()
    .find(|(removed_group, removed_leaf, _)| *removed_group == group && *removed_leaf == leaf)
    .map(|(_, _, replacement)| *replacement)
}

fn removal_message(subject: &str, replacement: &str) -> String {
  if replacement == NO_REPLACEMENT {
    format!("{subject} was removed in 0.31.0 and has no replacement. See docs/run/upgrade.md.")
  } else {
    format!("{subject} was removed in 0.31.0; use {replacement}. See docs/run/upgrade.md.")
  }
}

/// Explains a command line that uses an entry removed in 0.31.0, before argument
/// parsing reports it as an unknown command. `args` includes the program name.
/// Only the first command-group token and the leaf right after it are inspected,
/// so positional values such as `docs read <file> <heading>` are never mistaken for commands.
pub fn removed_command_error(args: &[String]) -> Option<String> {
  let position = args.iter().skip(1).position(|arg| REMOVED_GROUPS.contains(&arg.as_str()))? + 1;
  let group = args[position].as_str();
  let leaf = args.get(position + 1)?.as_str();
  if let Some(replacement) = removed_replacement(group, leaf) {
    return Some(removal_message(&format!("`calcit {group} {leaf}`"), replacement));
  }
  if REMOVED_JSON_SWITCH.contains(&(group, leaf)) && args[position + 2..].iter().any(|arg| arg == "--json") {
    return Some(removal_message(&format!("`calcit {group} {leaf} --json`"), "`--format json`"));
  }
  None
}

/// Explains an `edit transaction` operation that names an entry removed in 0.31.0.
pub fn removed_transaction_operation_error(index: usize, group: &str, subcommand: &str) -> Option<String> {
  let replacement = removed_replacement(group, subcommand)?;
  let replacement = match (group, subcommand) {
    ("tree", "batch-delete") => "one `tree delete` operation per path, highest index first".to_owned(),
    ("edit", "rm-example") => "an `edit examples` operation with the remaining examples".to_owned(),
    ("edit", "ns-doc") => "`calcit edit ns <ns>` with the changed `:ns` doc as its own guarded step".to_owned(),
    ("tree", _) => replacement.replacen("`calcit ", "`", 1),
    _ => replacement.to_owned(),
  };
  Some(format!(
    "Transaction operation {} uses `{group} {subcommand}`, which was removed in 0.31.0; use {replacement}. See docs/run/upgrade.md.",
    index + 1
  ))
}

/// Returns the one-line migration hint for a deprecated command that is kept
/// because ecosystem CI still calls it.
pub fn deprecated_command_hint(cli_args: &ToplevelCalcit) -> Option<String> {
  let (old, new) = match cli_args.subcommand.as_ref()? {
    CalcitCommand::Analyze(cmd) => match &cmd.subcommand {
      AnalyzeSubcommand::CheckTypes(_) => (
        "analyze check-types",
        "`calcit analyze weak-types --only coverage` (level filter: `--coverage-level none,partial,full`)",
      ),
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
    _ => return None,
  };
  Some(format_hint(&format!("`calcit {old}`"), new))
}

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

/// The analyzer selected by a merged `analyze` option that has no parsed entry of its own.
#[derive(Debug, Clone, PartialEq)]
pub enum AnalyzeView {
  /// `analyze weak-types --only dynamic-method`
  DynamicMethods(DynamicMethodsCommand),
  /// `analyze weak-types --only deprecated-call`
  DeprecatedCalls(DeprecatedCommand),
  /// `analyze call-graph --counts`
  CountCalls(CountCallsCommand),
  /// `analyze call-graph --diff <ref>`
  CallGraphDiff(CallGraphDiffCommand),
}

/// The action a `cursor` command performs once its merged form is resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum CursorAction {
  Set(CursorSetCommand),
  Show(CursorShowCommand),
  Clear,
  Parent,
  Child { index: Option<usize>, last: bool },
  Next(usize),
  Prev(usize),
  Back(usize),
  Forward(usize),
  Backward(usize),
  Push,
  Pop,
  SetAnchor,
  ClearAnchor,
  ShowRegion(String),
  ListMarks(String),
  SetMark(String),
  GotoMark(String),
  DropMark(String),
  ShowClipboard(String),
  Copy,
  Cut,
  Paste(String),
  ClearClipboard,
  Apply(CursorApplyCommand),
  SlurpNext,
  SlurpPrev,
  BarfLast,
  BarfFirst,
  Duplicate(String),
}

/// Resolves the `cursor move`, `anchor`, `marks`, `clipboard` and `apply` forms to the
/// action each one performs. Every invalid value is rejected here, before any file is touched.
pub fn normalize_cursor_subcommand(subcommand: &CursorSubcommand) -> Result<CursorAction, String> {
  match subcommand {
    CursorSubcommand::Move(o) => {
      let direction = o.direction.as_str();
      if !matches!(direction, "parent" | "child" | "next" | "prev" | "forward" | "backward" | "back") {
        return Err(format!(
          "Unsupported `cursor move {direction}`. Use: parent, child, next, prev, forward, backward, back."
        ));
      }
      if direction != "child" && (o.index.is_some() || o.last) {
        return Err(format!(
          "`cursor move {direction}` does not accept an index or --last; they belong to `child`."
        ));
      }
      if matches!(direction, "parent" | "child") && o.count.is_some() {
        return Err(format!("`cursor move {direction}` does not accept --count."));
      }
      let count = o.count.unwrap_or(1);
      Ok(match direction {
        "parent" => CursorAction::Parent,
        "child" => CursorAction::Child {
          index: o.index,
          last: o.last,
        },
        "next" => CursorAction::Next(count),
        "prev" => CursorAction::Prev(count),
        "forward" => CursorAction::Forward(count),
        "backward" => CursorAction::Backward(count),
        _ => CursorAction::Back(count),
      })
    }
    CursorSubcommand::Anchor(o) => match o.action.as_deref().unwrap_or("set") {
      "set" if o.format.is_none() => Ok(CursorAction::SetAnchor),
      "clear" if o.format.is_none() => Ok(CursorAction::ClearAnchor),
      "show" => Ok(CursorAction::ShowRegion(o.format.clone().unwrap_or_else(|| "human".to_owned()))),
      "set" | "clear" => Err("`cursor anchor set` and `cursor anchor clear` do not accept --format; it belongs to `show`.".to_owned()),
      other => Err(format!("Unsupported `cursor anchor {other}`. Use: set, clear, show.")),
    },
    CursorSubcommand::Marks(o) => {
      if o.args.len() > 2 {
        return Err(format!(
          "`cursor marks` takes an action and at most one mark name, got {} arguments.",
          o.args.len()
        ));
      }
      let action = o.args.first().map_or("list", String::as_str);
      if o.format.is_some() && action != "list" {
        return Err(format!("`cursor marks {action}` does not accept --format; it belongs to `list`."));
      }
      match (action, o.args.get(1)) {
        ("list", None) => Ok(CursorAction::ListMarks(o.format.clone().unwrap_or_else(|| "human".to_owned()))),
        ("list", Some(_)) => Err("`cursor marks list` does not accept a mark name.".to_owned()),
        ("set" | "go" | "drop", None) => Err(format!("`cursor marks {action}` requires a mark name.")),
        ("set", Some(name)) => Ok(CursorAction::SetMark(name.clone())),
        ("go", Some(name)) => Ok(CursorAction::GotoMark(name.clone())),
        ("drop", Some(name)) => Ok(CursorAction::DropMark(name.clone())),
        (other, _) => Err(format!("Unsupported `cursor marks {other}`. Use: list, set, go, drop.")),
      }
    }
    CursorSubcommand::Clipboard(o) => {
      let action = o.action.as_deref().unwrap_or("show");
      if o.format.is_some() && action != "show" {
        return Err(format!(
          "`cursor clipboard {action}` does not accept --format; it belongs to `show`."
        ));
      }
      if o.at.is_some() && action != "paste" {
        return Err(format!("`cursor clipboard {action}` does not accept --at; it belongs to `paste`."));
      }
      match action {
        "show" => Ok(CursorAction::ShowClipboard(o.format.clone().unwrap_or_else(|| "human".to_owned()))),
        "copy" => Ok(CursorAction::Copy),
        "cut" => Ok(CursorAction::Cut),
        "clear" => Ok(CursorAction::ClearClipboard),
        "paste" => Ok(CursorAction::Paste(o.at.clone().unwrap_or_else(|| "after".to_owned()))),
        other => Err(format!(
          "Unsupported `cursor clipboard {other}`. Use: show, copy, cut, paste, clear."
        )),
      }
    }
    CursorSubcommand::Apply(o) => {
      let operation = o.operation.as_str();
      if !matches!(operation, "slurp-next" | "slurp-prev" | "barf-last" | "barf-first" | "duplicate") {
        if o.at.is_some() {
          return Err(format!(
            "Cursor operation '{operation}' does not accept --at; it belongs to `duplicate`."
          ));
        }
        return Ok(CursorAction::Apply(o.clone()));
      }
      if o.file.is_some() || o.code.is_some() || o.input_format != calcit::cli_args::SyntaxInputFormat::Auto {
        return Err(format!(
          "Cursor operation '{operation}' does not accept --file, --code, or --input-format."
        ));
      }
      if o.at.is_some() && operation != "duplicate" {
        return Err(format!(
          "Cursor operation '{operation}' does not accept --at; it belongs to `duplicate`."
        ));
      }
      Ok(match operation {
        "slurp-next" => CursorAction::SlurpNext,
        "slurp-prev" => CursorAction::SlurpPrev,
        "barf-last" => CursorAction::BarfLast,
        "barf-first" => CursorAction::BarfFirst,
        _ => CursorAction::Duplicate(o.at.clone().unwrap_or_else(|| "after".to_owned())),
      })
    }
    CursorSubcommand::Set(o) => Ok(CursorAction::Set(o.clone())),
    CursorSubcommand::Show(o) => Ok(CursorAction::Show(o.clone())),
    CursorSubcommand::Clear(_) => Ok(CursorAction::Clear),
    CursorSubcommand::Push(_) => Ok(CursorAction::Push),
    CursorSubcommand::Pop(_) => Ok(CursorAction::Pop),
  }
}

/// Routes every merged `analyze` option to the analyzer that owns it. The
/// returned view, when present, takes precedence over the parsed subcommand.
pub fn normalize_consolidated_views(cli_args: ToplevelCalcit) -> Result<(ToplevelCalcit, Option<AnalyzeView>), String> {
  let (cli_args, view) = normalize_weak_types_view(cli_args)?;
  if view.is_some() {
    return Ok((cli_args, view));
  }
  let view = call_graph_view(&cli_args)?;
  Ok((cli_args, view))
}

/// Routes `analyze call-graph --counts` and `--diff <ref>` to the call-count
/// and call-graph comparison analyzers.
fn call_graph_view(cli_args: &ToplevelCalcit) -> Result<Option<AnalyzeView>, String> {
  let Some(CalcitCommand::Analyze(AnalyzeCommand {
    subcommand: AnalyzeSubcommand::CallGraph(options),
  })) = &cli_args.subcommand
  else {
    return Ok(None);
  };
  let options: &CallGraphCommand = options;
  let view = match (options.counts, &options.diff) {
    (true, Some(_)) => return Err("`analyze call-graph` accepts `--counts` or `--diff <ref>`, not both.".to_owned()),
    (false, None) => {
      if options.sort.is_some() {
        return Err("`--sort` requires `analyze call-graph --counts`.".to_owned());
      }
      return Ok(None);
    }
    (true, None) => {
      reject_flags(
        "analyze call-graph --counts",
        &[("--max-depth", options.max_depth != 0), ("--show-unused", options.show_unused)],
      )?;
      AnalyzeView::CountCalls(CountCallsCommand {
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
      AnalyzeView::CallGraphDiff(CallGraphDiffCommand {
        git_ref: git_ref.clone(),
        root: options.root.clone(),
        ns_prefix: options.ns_prefix.clone(),
        include_core: options.include_core,
        max_depth: options.max_depth,
      })
    }
  };
  Ok(Some(view))
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
  let views = [VIEW_COVERAGE, VIEW_DYNAMIC_METHOD, VIEW_DEPRECATED_CALL, VIEW_OPTION_NIL_ROUNDTRIP];
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

/// Routes `analyze weak-types --only <view>` to the analyzer that owns that
/// view. The coverage view shares the `analyze check-types` entry; the other
/// views are returned as an [`AnalyzeView`].
pub fn normalize_weak_types_view(cli_args: ToplevelCalcit) -> Result<(ToplevelCalcit, Option<AnalyzeView>), String> {
  let Some(CalcitCommand::Analyze(AnalyzeCommand {
    subcommand: AnalyzeSubcommand::WeakTypes(options),
  })) = &cli_args.subcommand
  else {
    return Ok((cli_args, None));
  };
  let view = view_kind(options.only.as_deref())?;
  if view.is_none() {
    if options.coverage_level.is_some() {
      return Err("`--coverage-level` requires `analyze weak-types --only coverage`.".to_owned());
    }
    return Ok((cli_args, None));
  }
  let options: &WeakTypesCommand = options;
  let evidence_flags = [
    ("--intent", options.intent.is_some()),
    ("--ffi-evidence", options.ffi_evidence),
    ("--schema-evidence", options.schema_evidence),
  ];
  if view == Some(VIEW_OPTION_NIL_ROUNDTRIP) {
    reject_options(VIEW_OPTION_NIL_ROUNDTRIP, &evidence_flags)?;
    reject_options(
      VIEW_OPTION_NIL_ROUNDTRIP,
      &[
        ("--incremental", options.incremental),
        ("--coverage-level", options.coverage_level.is_some()),
      ],
    )?;
    let mut options = options.clone();
    options.only = Some(VIEW_OPTION_NIL_ROUNDTRIP.to_owned());
    return Ok((
      ToplevelCalcit {
        subcommand: Some(CalcitCommand::Analyze(AnalyzeCommand {
          subcommand: AnalyzeSubcommand::WeakTypes(options),
        })),
        ..cli_args
      },
      None,
    ));
  }
  let analyze_view = match view {
    Some(VIEW_COVERAGE) => {
      reject_options(VIEW_COVERAGE, &evidence_flags)?;
      let subcommand = AnalyzeSubcommand::CheckTypes(CheckTypesCommand {
        ns: options.ns.clone(),
        ns_prefix: options.ns_prefix.clone(),
        only: options.coverage_level.clone(),
        format: options.format.clone(),
        deps: options.deps,
        summary_only: options.summary_only,
        incremental: options.incremental,
      });
      return Ok((
        ToplevelCalcit {
          subcommand: Some(CalcitCommand::Analyze(AnalyzeCommand { subcommand })),
          ..cli_args
        },
        None,
      ));
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
      AnalyzeView::DynamicMethods(DynamicMethodsCommand {
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
      AnalyzeView::DeprecatedCalls(DeprecatedCommand {
        ns: options.ns.clone(),
        ns_prefix: options.ns_prefix.clone(),
        format: options.format.clone(),
        deps: options.deps,
        summary_only: options.summary_only,
      })
    }
    _ => unreachable!("view kind was validated"),
  };
  Ok((cli_args, Some(analyze_view)))
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

  fn args(items: &[&str]) -> Vec<String> {
    std::iter::once("calcit").chain(items.iter().copied()).map(str::to_owned).collect()
  }

  #[test]
  fn type_policy_flags_report_deprecation_only_when_used() {
    use super::deprecated_type_policy_flag_hint as hint;
    assert!(hint(false, false).is_none());
    assert!(hint(true, false).unwrap().contains("`--strict-types`"));
    assert!(hint(false, true).unwrap().contains("`--compat-types`"));
    assert!(hint(true, true).unwrap().contains("`--strict-types` and `--compat-types`"));
  }

  #[test]
  fn weak_types_views_select_their_analyzers() {
    let (coverage, view) = normalize_weak_types_view(parse(&[
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
    assert_eq!(view, None);
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

    let (_, dynamic) =
      normalize_weak_types_view(parse(&["analyze", "weak-types", "--only", "dynamic-method", "--deps"])).expect("dynamic view");
    assert_eq!(
      dynamic,
      Some(AnalyzeView::DynamicMethods(DynamicMethodsCommand {
        format: "human".to_owned(),
        deps: true,
        summary_only: false,
        incremental: false,
      }))
    );

    let (_, deprecated) = normalize_weak_types_view(parse(&[
      "analyze",
      "weak-types",
      "--only",
      "deprecated-call",
      "--ns-prefix",
      "app.",
    ]))
    .expect("view");
    assert_eq!(
      deprecated,
      Some(AnalyzeView::DeprecatedCalls(DeprecatedCommand {
        ns: None,
        ns_prefix: Some("app.".to_owned()),
        format: "human".to_owned(),
        deps: false,
        summary_only: false,
      }))
    );
  }

  #[test]
  fn weak_types_match_kinds_stay_unchanged() {
    let cli_args = parse(&["analyze", "weak-types", "--only", "code-nil,code-dynamic"]);
    assert_eq!(normalize_weak_types_view(cli_args.clone()).expect("kinds"), (cli_args, None));
  }

  #[test]
  fn option_nil_view_keeps_the_shared_entry_and_normalizes_its_spelling() {
    let options = ["analyze", "weak-types", "--only", "option-nil-roundtrip", "--ns", "app.main"];
    let expected = parse(&options);
    assert_eq!(normalize_weak_types_view(expected.clone()).unwrap(), (expected.clone(), None));
    let padded = parse(&["analyze", "weak-types", "--only", " option-nil-roundtrip, ", "--ns", "app.main"]);
    assert_eq!(normalize_weak_types_view(padded).unwrap(), (expected, None));
  }

  #[test]
  fn weak_types_views_reject_mixed_or_unsupported_options() {
    for cli_args in [
      &["analyze", "weak-types", "--only", "coverage,code-nil"][..],
      &["analyze", "weak-types", "--only", "coverage,deprecated-call"][..],
      &["analyze", "weak-types", "--only", "dynamic-method", "--ns", "app.main"][..],
      &["analyze", "weak-types", "--only", "deprecated-call", "--incremental"][..],
      &["analyze", "weak-types", "--only", "coverage", "--intent", "unresolved"][..],
      &["analyze", "weak-types", "--coverage-level", "none"][..],
    ] {
      assert!(
        normalize_weak_types_view(parse(cli_args)).is_err(),
        "{cli_args:?} should be rejected"
      );
    }
  }

  #[test]
  fn kept_deprecated_entries_name_their_replacement() {
    for (cli_args, replacement) in [
      (&["analyze", "check-types"][..], "weak-types --only coverage"),
      (&["analyze", "check-public", "--ns", "app.lib"][..], "--check-only --ns"),
      (&["analyze", "quality"][..], "--check-only"),
    ] {
      let hint = deprecated_command_hint(&parse(cli_args)).expect("deprecated entry should have a hint");
      assert!(hint.starts_with("[Deprecated]") && hint.contains(replacement), "{hint}");
      assert!(!hint.contains('\n'), "hint should be one line: {hint}");
    }
    assert_eq!(deprecated_command_hint(&parse(&["analyze", "weak-types"])), None);
    assert_eq!(deprecated_command_hint(&parse(&["config", "modules"])), None);
    assert_eq!(deprecated_command_hint(&parse(&["analyze", "call-graph", "--counts"])), None);
  }

  #[test]
  fn removed_entries_explain_their_replacement() {
    for (cli_args, replacement) in [
      (&["query", "pkg"][..], "query config"),
      (&["--entry", "page", "calcit.cirru", "query", "modules"][..], "config modules"),
      (&["query", "host-procs"][..], "has no replacement"),
      (
        &["tree", "insert-before", "app.main/f", "--path", "0"][..],
        "tree insert --at before",
      ),
      (&["tree", "swap-prev", "app.main/f", "--path", "0"][..], "tree swap --with prev"),
      (&["tree", "batch-delete", "app.main/f", "--paths", "0"][..], "edit transaction"),
      (&["cursor", "parent"][..], "cursor move parent"),
      (&["cursor", "duplicate", "--at", "before"][..], "cursor apply duplicate"),
      (&["analyze", "count-calls"][..], "call-graph --counts"),
      (&["analyze", "call-graph-diff", "HEAD"][..], "call-graph --diff"),
      (&["analyze", "dynamic-methods"][..], "weak-types --only dynamic-method"),
      (&["analyze", "deprecated"][..], "weak-types --only deprecated-call"),
      (&["analyze", "js-escape", "a?"][..], "has no replacement"),
      (&["edit", "rm-example", "app.main/f", "0"][..], "edit examples"),
      (&["edit", "ns-doc", "app.main", "doc"][..], "edit ns <ns>"),
      (&["docs", "read-lines", "a.md"][..], "docs read <file> --start"),
      (&["cirru", "show-guide"][..], "docs read cirru-syntax.md"),
      (&["config", "version", "patch"][..], "caps version"),
      (&["query", "def", "app.main/f", "--json"][..], "`--format json`"),
      (&["tree", "show", "app.main/f", "--json"][..], "`--format json`"),
    ] {
      let error = removed_command_error(&args(cli_args)).expect("removed entry should be explained");
      assert!(error.contains("was removed in 0.31.0") && error.contains(replacement), "{error}");
      assert!(!error.contains('\n'), "message should be one line: {error}");
    }
    for cli_args in [
      &["query", "def", "app.main/f", "--format", "json"][..],
      &["query", "search-expr", "[\"defn\"]", "--json"][..],
      &["docs", "read", "query", "modules"][..],
      &["cursor", "move", "parent"][..],
      &["tree", "insert", "a/b", "--path", "0", "--at", "after"][..],
      &["analyze", "call-graph", "--counts"][..],
      &["--check-only"][..],
      &["ffi", "export", "--json"][..],
    ] {
      assert_eq!(removed_command_error(&args(cli_args)), None, "{cli_args:?}");
    }
  }

  #[test]
  fn call_graph_options_select_their_analyzers() {
    let view = |cli_args: &[&str]| normalize_consolidated_views(parse(cli_args)).expect("call-graph option").1;
    assert_eq!(
      view(&[
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
      ]),
      Some(AnalyzeView::CountCalls(CountCallsCommand {
        root: Some("app.main/main!".to_owned()),
        ns_prefix: None,
        include_core: true,
        format: "json".to_owned(),
        sort: "name".to_owned(),
      }))
    );
    assert_eq!(
      view(&[
        "analyze",
        "call-graph",
        "--diff",
        "HEAD~1",
        "--max-depth",
        "3",
        "--ns-prefix",
        "app."
      ]),
      Some(AnalyzeView::CallGraphDiff(CallGraphDiffCommand {
        git_ref: "HEAD~1".to_owned(),
        root: None,
        ns_prefix: Some("app.".to_owned()),
        include_core: false,
        max_depth: 3,
      }))
    );
    let plain = parse(&["analyze", "call-graph", "--show-unused"]);
    assert_eq!(
      normalize_consolidated_views(plain.clone()).expect("plain call-graph"),
      (plain, None)
    );
    for cli_args in [
      &["analyze", "call-graph", "--counts", "--diff", "HEAD"][..],
      &["analyze", "call-graph", "--sort", "name"][..],
      &["analyze", "call-graph", "--counts", "--max-depth", "2"][..],
      &["analyze", "call-graph", "--counts", "--show-unused"][..],
      &["analyze", "call-graph", "--diff", "HEAD", "--format", "json"][..],
      &["analyze", "call-graph", "--diff", "HEAD", "--show-unused"][..],
    ] {
      assert!(
        normalize_consolidated_views(parse(cli_args)).is_err(),
        "{cli_args:?} should be rejected"
      );
    }
  }

  #[test]
  fn removed_transaction_operations_name_their_replacement() {
    for (group, subcommand, replacement) in [
      ("tree", "batch-delete", "tree delete"),
      ("tree", "insert-before", "`tree insert --at before`"),
      ("tree", "append-child", "`tree insert --at append-child`"),
      ("tree", "swap-next", "`tree swap --with next`"),
      ("edit", "rm-example", "edit examples"),
      ("edit", "ns-doc", "edit ns <ns>"),
      ("config", "version", "caps version"),
    ] {
      let error = removed_transaction_operation_error(1, group, subcommand).expect("removed operation should be explained");
      assert!(
        error.starts_with("Transaction operation 2") && error.contains("removed in 0.31.0") && error.contains(replacement),
        "{error}"
      );
    }
    assert_eq!(removed_transaction_operation_error(0, "edit", "examples"), None);
    assert_eq!(removed_transaction_operation_error(0, "tree", "insert"), None);
  }

  #[test]
  fn cursor_forms_resolve_to_their_actions() {
    let cursor = |cli_args: &[&str]| match parse(cli_args).subcommand {
      Some(CalcitCommand::Cursor(cmd)) => normalize_cursor_subcommand(&cmd.subcommand),
      other => panic!("expected cursor command, got {other:?}"),
    };
    assert_eq!(cursor(&["cursor", "move", "parent"]), Ok(CursorAction::Parent));
    assert_eq!(
      cursor(&["cursor", "move", "child", "2"]),
      Ok(CursorAction::Child {
        index: Some(2),
        last: false
      })
    );
    assert_eq!(cursor(&["cursor", "move", "next", "--count", "3"]), Ok(CursorAction::Next(3)));
    assert_eq!(cursor(&["cursor", "anchor"]), Ok(CursorAction::SetAnchor));
    assert_eq!(
      cursor(&["cursor", "anchor", "show"]),
      Ok(CursorAction::ShowRegion("human".to_owned()))
    );
    assert_eq!(
      cursor(&["cursor", "marks", "go", "start"]),
      Ok(CursorAction::GotoMark("start".to_owned()))
    );
    assert_eq!(
      cursor(&["cursor", "clipboard", "paste"]),
      Ok(CursorAction::Paste("after".to_owned()))
    );
    assert_eq!(
      cursor(&["cursor", "apply", "duplicate", "--at", "before"]),
      Ok(CursorAction::Duplicate("before".to_owned()))
    );
    assert!(cursor(&["cursor", "move", "sideways"]).is_err());
    assert!(cursor(&["cursor", "marks", "set"]).is_err());
    assert!(cursor(&["cursor", "clipboard", "copy", "--at", "before"]).is_err());
  }
}
