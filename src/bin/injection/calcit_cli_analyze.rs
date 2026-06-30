//! Analyze builtins: call graph, effects graph, call counts.

use calcit::calcit::{Calcit, CalcitErr};
use calcit::call_stack::CallStackList;
use calcit::util;
use std::sync::Arc;

use super::calcit_cli::load_calcit_snapshot_with_deps;
use super::calcit_cli_args::resolve_cli_args;
use super::calcit_cli_program::prepare_program_from_snapshot_file;
use super::calcit_cli_specs::{
  ANALYZE_CALL_GRAPH, ANALYZE_CHECK_TYPES, ANALYZE_COUNT_CALLS, ANALYZE_EFFECTS_GRAPH, ANALYZE_WEAK_TYPES,
};

pub fn analyze_call_graph(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/analyze-call-graph", &xs, ANALYZE_CALL_GRAPH)?;
  let file_path = args.file_path()?;
  let root = args.optional_string("root");
  let format = args.string("format")?;
  let max_depth = args.usize("max-depth")?;
  let include_core = args.bool("include-core")?;
  let ns_prefix = args.optional_string("ns-prefix");
  let show_unused = args.bool("show-unused")?;

  let entries = prepare_program_from_snapshot_file(&file_path)?;
  let (entry_ns, entry_def) = match root {
    Some(r) => util::string::extract_ns_def(&r).map_err(CalcitErr::from)?,
    None => (entries.init_ns.to_string(), entries.init_def.to_string()),
  };

  let result = calcit::call_tree::analyze_call_graph(&entry_ns, &entry_def, include_core, max_depth, show_unused, None, ns_prefix)
    .map_err(CalcitErr::from)?;

  let output = if format == "json" {
    calcit::call_tree::format_as_json(&result).map_err(CalcitErr::from)?
  } else {
    calcit::call_tree::format_for_llm(&result)
  };
  Ok(Calcit::Str(Arc::from(output)))
}

pub fn analyze_effects_graph(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/analyze-effects-graph", &xs, ANALYZE_EFFECTS_GRAPH)?;
  let file_path = args.file_path()?;
  let root = args.optional_string("root");
  let format = args.string("format")?;
  let max_depth = args.usize("max-depth")?;
  let include_core = args.bool("include-core")?;
  let ns_prefix = args.optional_string("ns-prefix");
  let detail_raw = args.string("detail")?;

  let entries = prepare_program_from_snapshot_file(&file_path)?;
  let (entry_ns, entry_def) = match root {
    Some(r) => util::string::extract_ns_def(&r).map_err(CalcitErr::from)?,
    None => (entries.init_ns.to_string(), entries.init_def.to_string()),
  };

  let detail = match detail_raw.as_str() {
    "full" => calcit::effects_graph::EffectsGraphDetail::Full,
    "minimal" => calcit::effects_graph::EffectsGraphDetail::Minimal,
    "summary" | "text" => calcit::effects_graph::EffectsGraphDetail::Summary,
    other => {
      return Err(CalcitErr::from(format!(
        "analyze-effects-graph: unknown detail `{other}`, use summary|full|minimal"
      )));
    }
  };

  let result = calcit::effects_graph::analyze_effects_graph(&entry_ns, &entry_def, include_core, max_depth, ns_prefix.clone(), detail)
    .map_err(CalcitErr::from)?;

  let output = if format == "json" {
    calcit::effects_graph::format_as_json(&result).map_err(CalcitErr::from)?
  } else {
    calcit::effects_graph::format_as_ste_tree(&result, true)
  };
  Ok(Calcit::Str(Arc::from(output)))
}

pub fn analyze_count_calls(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  let args = resolve_cli_args("calcit.cli/analyze-count-calls", &xs, ANALYZE_COUNT_CALLS)?;
  let file_path = args.file_path()?;
  let root = args.optional_string("root");
  let format = args.string("format")?;
  let include_core = args.bool("include-core")?;
  let ns_prefix = args.optional_string("ns-prefix");
  let sort = args.string("sort")?;

  let entries = prepare_program_from_snapshot_file(&file_path)?;
  let (entry_ns, entry_def) = match root {
    Some(r) => util::string::extract_ns_def(&r).map_err(CalcitErr::from)?,
    None => (entries.init_ns.to_string(), entries.init_def.to_string()),
  };

  let result = calcit::call_tree::count_calls(&entry_ns, &entry_def, include_core, ns_prefix).map_err(CalcitErr::from)?;

  let output = if format == "json" {
    calcit::call_tree::format_count_as_json(&result).map_err(CalcitErr::from)?
  } else {
    calcit::call_tree::format_count_for_display(&result, &sort)
  };
  Ok(Calcit::Str(Arc::from(output)))
}

pub fn analyze_check_types(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  use calcit::cli_args::CheckTypesCommand;

  let args = resolve_cli_args("calcit.cli/analyze-check-types", &xs, ANALYZE_CHECK_TYPES)?;
  let file_path = args.file_path()?;
  let options = CheckTypesCommand {
    ns: args.optional_string("namespace"),
    ns_prefix: args.optional_string("ns-prefix"),
    only: args.optional_string("only-levels"),
    deps: args.bool("include-deps")?,
  };
  let snapshot = load_calcit_snapshot_with_deps(&file_path, true)?;
  let text = crate::type_coverage::format_check_types(&options, &snapshot).map_err(CalcitErr::from)?;
  Ok(Calcit::Str(Arc::from(text)))
}

pub fn analyze_weak_types(xs: Vec<Calcit>, _call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
  use calcit::cli_args::WeakTypesCommand;

  let args = resolve_cli_args("calcit.cli/analyze-weak-types", &xs, ANALYZE_WEAK_TYPES)?;
  let file_path = args.file_path()?;
  let options = WeakTypesCommand {
    ns: args.optional_string("namespace"),
    ns_prefix: args.optional_string("ns-prefix"),
    only: args.optional_string("only-kinds"),
    deps: args.bool("include-deps")?,
  };
  let snapshot = load_calcit_snapshot_with_deps(&file_path, true)?;
  let text = crate::type_coverage::format_weak_types(&options, &snapshot).map_err(CalcitErr::from)?;
  Ok(Calcit::Str(Arc::from(text)))
}
