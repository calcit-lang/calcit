//! Bin-local helpers for `calcit.cli/*` echo output.

pub use calcit::builtins::cli_options::{
  CliArgDefault, CliArgKind, CliArgSpec, ResolvedCliArgs, format_cli_docs_hint, parse_target, spec,
};

use calcit::calcit::Calcit;
use cirru_edn::EdnTag;
use colored::Colorize;
use rpds::HashTrieMapSync;

pub fn resolve_cli_args(
  proc_name: &str,
  xs: &[Calcit],
  specs: &'static [CliArgSpec],
) -> Result<ResolvedCliArgs, calcit::calcit::CalcitErr> {
  let resolved = calcit::builtins::cli_options::resolve_cli_args(proc_name, xs, specs)?;
  echo_resolved_call(&resolved);
  Ok(resolved)
}

pub fn format_cli_call_echo(args: &ResolvedCliArgs) -> String {
  let mut parts = vec!["$ {}".to_string()];
  for (key, value, is_default) in args.iter_provided_entries() {
    let rendered = format_calcit_value_for_echo(value);
    let entry = if is_default {
      format!("(:{key} {})", rendered.dimmed())
    } else {
      format!("(:{key} {rendered})")
    };
    parts.push(entry);
  }
  format!("{} {}", args.proc_name(), parts.join(" "))
}

fn format_calcit_value_for_echo(value: &Calcit) -> String {
  match value {
    Calcit::Str(s) => {
      let text = s.strip_prefix('|').unwrap_or(s);
      if text.contains(' ') || text.is_empty() {
        format!("\"|{text}\"")
      } else {
        format!("|{text}")
      }
    }
    Calcit::Bool(b) => b.to_string(),
    Calcit::Number(n) if n.fract() == 0.0 => format!("{}", *n as i64),
    Calcit::Number(n) => n.to_string(),
    Calcit::Tag(tag) => format!(":{}", tag.ref_str()),
    Calcit::Nil => "nil".to_string(),
    other => other.lisp_str(),
  }
}

fn echo_resolved_call(args: &ResolvedCliArgs) {
  if calcit::quiet_tool_output() {
    return;
  }
  eprintln!("{}", format_cli_call_echo(args).dimmed());
}

/// Build a single options-map argument for tests and snippets.
#[allow(dead_code)]
pub fn build_cli_opts(pairs: &[(&str, Calcit)]) -> Vec<Calcit> {
  let mut map = HashTrieMapSync::new_sync();
  for (k, v) in pairs {
    map.insert_mut(Calcit::Tag(EdnTag::new(*k)), v.clone());
  }
  vec![Calcit::Map(map)]
}

/// Empty options map `{}` as sole argument.
#[allow(dead_code)]
pub fn empty_cli_opts() -> Vec<Calcit> {
  vec![Calcit::Map(HashTrieMapSync::new_sync())]
}

#[cfg(test)]
mod tests {
  use super::*;
  use calcit::builtins::cli_options::{CliOptionDefault, CliOptionKind, cli_option};
  use rpds::HashTrieMap;
  use std::sync::Arc;

  fn map_with(pairs: &[(&str, Calcit)]) -> Calcit {
    let mut map = HashTrieMap::new_sync();
    for (k, v) in pairs {
      map.insert_mut(Calcit::Tag(EdnTag::new(*k)), v.clone());
    }
    Calcit::Map(map)
  }

  static PEEK_SPECS: &[CliArgSpec] = &[
    cli_option("file-path", CliOptionKind::String, true, None),
    cli_option("target", CliOptionKind::String, true, None),
    cli_option("lines", CliOptionKind::Usize, false, Some(CliOptionDefault::Usize(5))),
  ];

  #[test]
  fn resolves_required_and_default() {
    calcit::set_quiet_tool_output(true);
    let xs = vec![map_with(&[
      ("file-path", Calcit::Str(Arc::from("|a.cirru"))),
      ("target", Calcit::Str(Arc::from("|app.main/main!"))),
    ])];
    let args = resolve_cli_args("calcit.cli/peek-def", &xs, PEEK_SPECS).unwrap();
    assert_eq!(args.string("file-path").unwrap(), "a.cirru");
    assert_eq!(args.usize("lines").unwrap(), 5);
  }

  #[test]
  fn rejects_unknown_keys() {
    calcit::set_quiet_tool_output(true);
    let xs = vec![map_with(&[
      ("oops", Calcit::Str(Arc::from("|x"))),
      ("file-path", Calcit::Str(Arc::from("|a.cirru"))),
    ])];
    let err = resolve_cli_args("calcit.cli/peek-def", &xs, PEEK_SPECS).unwrap_err();
    assert!(err.to_string().contains("unknown option"));
  }
}
