//! Map-based keyword options for registered CLI procs (`calcit.cli/*`).
//!
//! Specs are defined once in Rust and used at both preprocess check time and runtime.

use std::collections::HashMap;
use std::sync::Arc;

use rpds::HashTrieMapSync;

use crate::calcit::{CORE_NS, Calcit, CalcitErr, CalcitImport, CalcitProc, CalcitSyntax};
use crate::data::cirru::{calcit_data_to_cirru, calcit_to_cirru};
use cirru_parser::Cirru;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliOptionKind {
  String,
  Bool,
  Usize,
  /// Quoted Cirru AST (`quote |leaf` or `quote $ expr ...`), not a plain string path.
  CirruQuote,
  /// List of strings (`(:paths $ [] |3.2 |4.1)`).
  StringList,
}

#[derive(Debug, Clone, Copy)]
pub enum CliOptionDefault {
  String(&'static str),
  Bool(bool),
  Usize(usize),
}

#[derive(Debug, Clone, Copy)]
pub struct CliOptionSpec {
  pub key: &'static str,
  pub kind: CliOptionKind,
  pub required: bool,
  pub default: Option<CliOptionDefault>,
}

pub const fn cli_option(key: &'static str, kind: CliOptionKind, required: bool, default: Option<CliOptionDefault>) -> CliOptionSpec {
  CliOptionSpec {
    key,
    kind,
    required,
    default,
  }
}

pub type CliArgKind = CliOptionKind;
pub type CliArgDefault = CliOptionDefault;
pub type CliArgSpec = CliOptionSpec;
pub use cli_option as spec;

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliOptionValidationError {
  UnknownKeys(Vec<String>),
  MissingRequired(&'static str),
  TypeMismatch { key: String, expected: &'static str, got: String },
}

#[derive(Debug, Clone)]
pub struct CliOptionCheckIssue {
  pub message: String,
  pub code: &'static str,
}

#[derive(Debug, Clone)]
pub struct ResolvedCliArgs {
  proc_name: Arc<str>,
  values: HashMap<String, Calcit>,
  specs: &'static [CliOptionSpec],
  defaulted_keys: Vec<&'static str>,
}

pub fn check_cli_options_map(proc_name: &str, arg: &Calcit, specs: &[CliOptionSpec]) -> Vec<CliOptionCheckIssue> {
  let Some(entries) = collect_map_entry_refs(arg) else {
    return Vec::new();
  };

  collect_validation_errors(proc_name, &entries, specs, true)
    .into_iter()
    .map(|err| validation_error_to_issue(proc_name, err, specs))
    .collect()
}

pub fn resolve_cli_args(proc_name: &str, xs: &[Calcit], specs: &'static [CliOptionSpec]) -> Result<ResolvedCliArgs, CalcitErr> {
  let map = extract_options_map(proc_name, xs)?;
  let entries: Vec<(String, &Calcit)> = map
    .iter()
    .filter_map(|(key, value)| calcit_key_to_string(key).map(|key_str| (key_str, value)))
    .collect();

  let errors = collect_validation_errors(proc_name, &entries, specs, false);
  if let Some(first) = errors.first() {
    return Err(CalcitErr::from(validation_error_message(proc_name, first.clone(), specs)));
  }

  let mut values = HashMap::with_capacity(specs.len());
  let mut defaulted_keys = Vec::new();

  for (key, value) in entries {
    values.insert(key, value.clone());
  }

  for spec in specs {
    if values.contains_key(spec.key) {
      continue;
    }
    if let Some(default) = spec.default {
      values.insert(spec.key.to_string(), default_to_calcit(default));
      defaulted_keys.push(spec.key);
    }
  }

  Ok(ResolvedCliArgs {
    proc_name: Arc::from(proc_name),
    values,
    specs,
    defaulted_keys,
  })
}

fn collect_validation_errors(
  proc_name: &str,
  entries: &[(String, &Calcit)],
  specs: &[CliOptionSpec],
  strict_cirru_quote: bool,
) -> Vec<CliOptionValidationError> {
  let mut errors = Vec::new();
  let mut provided: HashMap<&str, &Calcit> = HashMap::with_capacity(entries.len());
  let mut unknown_keys = Vec::new();

  for (key, value) in entries {
    let Some(spec) = specs.iter().find(|s| s.key == key.as_str()) else {
      unknown_keys.push(key.clone());
      continue;
    };
    provided.insert(spec.key, value);
    if !value_matches_kind(value, spec.kind, strict_cirru_quote) {
      errors.push(CliOptionValidationError::TypeMismatch {
        key: key.clone(),
        expected: kind_label(spec.kind),
        got: value.lisp_str(),
      });
    }
  }

  if !unknown_keys.is_empty() {
    unknown_keys.sort();
    unknown_keys.dedup();
    errors.push(CliOptionValidationError::UnknownKeys(unknown_keys));
  }

  for spec in specs {
    if provided.contains_key(spec.key) || spec.default.is_some() {
      continue;
    }
    if spec.required {
      errors.push(CliOptionValidationError::MissingRequired(spec.key));
    }
  }

  let _ = proc_name;
  errors
}

fn validation_error_to_issue(proc_name: &str, err: CliOptionValidationError, specs: &[CliOptionSpec]) -> CliOptionCheckIssue {
  let code = match &err {
    CliOptionValidationError::UnknownKeys(_) => "W_CLI_OPTION_UNKNOWN_KEY",
    CliOptionValidationError::MissingRequired(_) => "W_CLI_OPTION_MISSING_REQUIRED",
    CliOptionValidationError::TypeMismatch { .. } => "W_CLI_OPTION_TYPE_MISMATCH",
  };
  CliOptionCheckIssue {
    message: validation_error_message(proc_name, err, specs),
    code,
  }
}

fn validation_error_message(proc_name: &str, err: CliOptionValidationError, specs: &[CliOptionSpec]) -> String {
  match err {
    CliOptionValidationError::UnknownKeys(keys) => {
      let known: Vec<&str> = specs.iter().map(|s| s.key).collect();
      format!(
        "{proc_name}: unknown option key(s): {}. Expected: {}",
        keys.join(", "),
        known.join(", ")
      )
    }
    CliOptionValidationError::MissingRequired(key) => format!("{proc_name}: missing required option `:{key}`"),
    CliOptionValidationError::TypeMismatch { key, expected, got } => {
      format!("{proc_name}: option `:{key}` expected {expected}, got {got}")
    }
  }
}

fn collect_map_entry_refs(arg: &Calcit) -> Option<Vec<(String, &Calcit)>> {
  match arg {
    Calcit::Map(map) => {
      let mut entries = Vec::with_capacity(map.size());
      for (key, value) in map.iter() {
        let key_str = calcit_key_to_string(key)?;
        entries.push((key_str, value));
      }
      Some(entries)
    }
    Calcit::List(list) => {
      let Some(Calcit::Proc(CalcitProc::NativeMap)) = list.first() else {
        return None;
      };
      let items: Vec<&Calcit> = list.iter().skip(1).collect();
      if items.len() % 2 != 0 {
        return None;
      }
      let mut entries = Vec::with_capacity(items.len() / 2);
      for chunk in items.chunks(2) {
        let key_str = calcit_key_to_string(chunk[0])?;
        entries.push((key_str, chunk[1]));
      }
      Some(entries)
    }
    _ => None,
  }
}

fn extract_options_map<'a>(proc_name: &str, xs: &'a [Calcit]) -> Result<&'a HashTrieMapSync<Calcit, Calcit>, CalcitErr> {
  match xs.len() {
    0 => Err(CalcitErr::from(format!(
      "{proc_name}: expected options map as sole argument, e.g. `$ {{}} (:file-path |path)`"
    ))),
    1 => match &xs[0] {
      Calcit::Map(map) => Ok(map),
      Calcit::Nil => Err(CalcitErr::from(format!(
        "{proc_name}: expected options map, got nil. Use `$ {{}}` with keyword entries"
      ))),
      other => Err(CalcitErr::from(format!(
        "{proc_name}: expected options map, got {}",
        other.lisp_str()
      ))),
    },
    n => Err(CalcitErr::from(format!(
      "{proc_name}: expected 1 options-map argument, got {n}. Use `$ {{}} (:key value)` instead of positional args"
    ))),
  }
}

fn default_to_calcit(default: CliOptionDefault) -> Calcit {
  match default {
    CliOptionDefault::String(s) => Calcit::Str(Arc::from(s)),
    CliOptionDefault::Bool(b) => Calcit::Bool(b),
    CliOptionDefault::Usize(n) => Calcit::Number(n as f64),
  }
}

fn calcit_key_to_string(key: &Calcit) -> Option<String> {
  match key {
    Calcit::Tag(tag) => Some(tag.ref_str().trim_start_matches(':').to_string()),
    Calcit::Str(s) => Some(strip_calcit_string(s)),
    _ => None,
  }
}

fn strip_calcit_string(s: &str) -> String {
  s.strip_prefix('|').unwrap_or(s).to_string()
}

fn kind_label(kind: CliOptionKind) -> &'static str {
  match kind {
    CliOptionKind::String => "string",
    CliOptionKind::Bool => "boolean",
    CliOptionKind::Usize => "unsigned integer",
    CliOptionKind::CirruQuote => "cirru-quote",
    CliOptionKind::StringList => "list of strings",
  }
}

fn value_matches_kind(value: &Calcit, kind: CliOptionKind, strict_cirru_quote: bool) -> bool {
  match kind {
    CliOptionKind::String => matches!(value, Calcit::Str(_) | Calcit::Tag(_)),
    CliOptionKind::Bool => match value {
      Calcit::Bool(_) | Calcit::Nil => true,
      Calcit::Str(s) => matches!(strip_calcit_string(s).as_str(), "true" | "false"),
      _ => false,
    },
    CliOptionKind::Usize => match value {
      Calcit::Number(n) => *n >= 0.0 && n.fract() == 0.0,
      Calcit::Str(s) => strip_calcit_string(s).parse::<usize>().is_ok(),
      _ => false,
    },
    CliOptionKind::CirruQuote => {
      if strict_cirru_quote {
        is_cirru_quote_source(value)
      } else {
        is_cirru_quote_runtime(value)
      }
    }
    CliOptionKind::StringList => is_string_list_value(value),
  }
}

fn is_list_constructor_head(value: &Calcit) -> bool {
  matches!(value, Calcit::Proc(CalcitProc::List))
    || matches!(value, Calcit::Symbol { sym, .. } if sym.as_ref() == "[]")
    || matches!(value, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "[]")
}

fn calcit_string_item(value: &Calcit) -> Option<String> {
  match value {
    Calcit::Str(s) => Some(strip_calcit_string(s)),
    Calcit::Tag(tag) => Some(tag.ref_str().trim_start_matches(':').to_string()),
    _ => None,
  }
}

fn extract_string_list_items(value: &Calcit) -> Result<Vec<String>, String> {
  let Calcit::List(list) = value else {
    return Err(format!("expected list, got {}", value.lisp_str()));
  };
  let raw: Vec<_> = list.iter().collect();
  let data = if raw.first().is_some_and(|head| is_list_constructor_head(head)) {
    &raw[1..]
  } else {
    raw.as_slice()
  };
  let mut items = Vec::with_capacity(data.len());
  for item in data {
    let Some(text) = calcit_string_item(item) else {
      return Err(format!("expected string list item, got {}", item.lisp_str()));
    };
    items.push(text);
  }
  Ok(items)
}

fn is_string_list_value(value: &Calcit) -> bool {
  extract_string_list_items(value).is_ok()
}

fn extract_string_list_payload(proc_name: &str, key: &str, value: &Calcit) -> Result<Vec<String>, CalcitErr> {
  extract_string_list_items(value).map_err(|detail| {
    CalcitErr::from(format!(
      "{proc_name}: option `:{key}` expected list of strings, got {} ({detail})",
      value.lisp_str()
    ))
  })
}

fn is_quote_call_head(head: &Calcit) -> bool {
  matches!(head, Calcit::Syntax(CalcitSyntax::Quote, _))
    || matches!(head, Calcit::Symbol { sym, .. } if sym.as_ref() == "quote")
    || matches!(head, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == CORE_NS && &**def == "quote")
}

fn quote_call_payload(value: &Calcit) -> Option<&Calcit> {
  let Calcit::List(list) = value else {
    return None;
  };
  let mut iter = list.iter();
  let head = iter.next()?;
  let payload = iter.next()?;
  if iter.next().is_some() || !is_quote_call_head(head) {
    return None;
  }
  Some(payload)
}

fn is_cirru_quote_source(value: &Calcit) -> bool {
  matches!(value, Calcit::CirruQuote(_)) || quote_call_payload(value).is_some()
}

fn can_convert_to_cirru(value: &Calcit) -> bool {
  calcit_data_to_cirru(value).is_ok() || calcit_to_cirru(value).is_ok()
}

fn is_cirru_quote_runtime(value: &Calcit) -> bool {
  is_cirru_quote_source(value) || can_convert_to_cirru(value)
}

fn calcit_to_cirru_payload(value: &Calcit) -> Result<Cirru, String> {
  calcit_data_to_cirru(value).or_else(|_| calcit_to_cirru(value))
}

fn extract_cirru_quote_payload(proc_name: &str, key: &str, value: &Calcit) -> Result<Cirru, CalcitErr> {
  match value {
    Calcit::CirruQuote(code) => Ok(code.clone()),
    value if quote_call_payload(value).is_some() => {
      let payload = quote_call_payload(value).expect("checked above");
      calcit_to_cirru_payload(payload)
        .map_err(|e| CalcitErr::from(format!("{proc_name}: option `:{key}` invalid cirru-quote payload: {e}")))
    }
    other => calcit_to_cirru_payload(other).map_err(|e| {
      CalcitErr::from(format!(
        "{proc_name}: option `:{key}` expected cirru-quote, got {} ({e})",
        other.lisp_str()
      ))
    }),
  }
}

impl ResolvedCliArgs {
  pub fn string(&self, key: &str) -> Result<String, CalcitErr> {
    let value = self.get_raw(key)?;
    match value {
      Calcit::Str(s) => Ok(strip_calcit_string(s)),
      Calcit::Tag(tag) => Ok(tag.ref_str().trim_start_matches(':').to_string()),
      other => Err(self.type_err(key, "string", other)),
    }
  }

  /// Resolve snapshot file path from `(:file-path ...)` or the host `cr` entry file.
  pub fn file_path(&self) -> Result<String, CalcitErr> {
    if let Some(value) = self.values.get("file-path") {
      return match value {
        Calcit::Str(s) => Ok(strip_calcit_string(s)),
        Calcit::Tag(tag) => Ok(tag.ref_str().trim_start_matches(':').to_string()),
        other => Err(self.type_err("file-path", "string", other)),
      };
    }
    crate::host_snapshot_file().ok_or_else(|| {
      CalcitErr::from(format!(
        "{}: missing option `:file-path` and no host snapshot file is set. \
         Pass `(:file-path |path.cirru)` or run via `cr <file.cirru> exec`",
        self.proc_name
      ))
    })
  }

  pub fn string_list(&self, key: &str) -> Result<Vec<String>, CalcitErr> {
    let value = self.get_raw(key)?;
    extract_string_list_payload(&self.proc_name, key, value)
  }

  pub fn cirru_quote(&self, key: &str) -> Result<Cirru, CalcitErr> {
    let value = self.get_raw(key)?;
    extract_cirru_quote_payload(&self.proc_name, key, value)
  }

  pub fn try_cirru_quote(&self, key: &str) -> Result<Option<Cirru>, CalcitErr> {
    match self.values.get(key) {
      None => Ok(None),
      Some(value) => extract_cirru_quote_payload(&self.proc_name, key, value).map(Some),
    }
  }

  pub fn optional_string(&self, key: &str) -> Option<String> {
    self.values.get(key).and_then(|v| match v {
      Calcit::Str(s) if !strip_calcit_string(s).is_empty() => Some(strip_calcit_string(s)),
      Calcit::Tag(tag) => Some(tag.ref_str().trim_start_matches(':').to_string()),
      Calcit::Nil => None,
      _ => None,
    })
  }

  pub fn bool(&self, key: &str) -> Result<bool, CalcitErr> {
    let value = self.get_raw(key)?;
    match value {
      Calcit::Bool(b) => Ok(*b),
      Calcit::Str(s) => match strip_calcit_string(s).as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(CalcitErr::from(format!(
          "{}: option `:{key}` expected boolean, got string `{other}`",
          self.proc_name
        ))),
      },
      Calcit::Nil => Ok(false),
      other => Err(self.type_err(key, "boolean", other)),
    }
  }

  pub fn usize(&self, key: &str) -> Result<usize, CalcitErr> {
    let value = self.get_raw(key)?;
    match value {
      Calcit::Number(n) if *n >= 0.0 && n.fract() == 0.0 => Ok(*n as usize),
      Calcit::Number(n) => Err(CalcitErr::from(format!(
        "{}: option `:{key}` expected unsigned integer, got number `{n}`",
        self.proc_name
      ))),
      Calcit::Str(s) => strip_calcit_string(s).parse::<usize>().map_err(|_| {
        CalcitErr::from(format!(
          "{}: option `:{key}` expected unsigned integer, got string `{s}`",
          self.proc_name
        ))
      }),
      other => Err(self.type_err(key, "unsigned integer", other)),
    }
  }

  pub fn target(&self, key: &str) -> Result<(String, String), CalcitErr> {
    let raw = self.string(key)?;
    parse_target(&self.proc_name, &raw)
  }

  pub fn specs(&self) -> &'static [CliOptionSpec] {
    self.specs
  }

  pub fn defaulted_keys(&self) -> &[&'static str] {
    &self.defaulted_keys
  }

  pub fn proc_name(&self) -> &str {
    &self.proc_name
  }

  pub fn iter_provided_entries(&self) -> impl Iterator<Item = (&'static str, &Calcit, bool)> + '_ {
    self.specs.iter().filter_map(|spec| {
      self
        .values
        .get(spec.key)
        .map(|value| (spec.key, value, self.defaulted_keys.contains(&spec.key)))
    })
  }

  fn get_raw(&self, key: &str) -> Result<&Calcit, CalcitErr> {
    self
      .values
      .get(key)
      .ok_or_else(|| CalcitErr::from(format!("{}: missing option `:{key}`", self.proc_name)))
  }

  fn type_err(&self, key: &str, expected: &str, got: &Calcit) -> CalcitErr {
    CalcitErr::from(format!(
      "{}: option `:{key}` expected {expected}, got {}",
      self.proc_name,
      got.lisp_str()
    ))
  }
}

pub fn parse_target(fn_name: &str, target: &str) -> Result<(String, String), CalcitErr> {
  let (ns, def) = target
    .split_once('/')
    .ok_or_else(|| CalcitErr::from(format!("{fn_name}: expected target in `ns/def` format, got `{target}`")))?;
  if ns.is_empty() || def.is_empty() {
    return Err(CalcitErr::from(format!(
      "{fn_name}: expected non-empty namespace and definition in `ns/def`, got `{target}`"
    )));
  }
  Ok((ns.to_string(), def.to_string()))
}

pub fn format_cli_docs_hint(proc_name: &str, specs: &[CliOptionSpec], result_hint: &str) -> String {
  let opts: Vec<String> = specs
    .iter()
    .map(|s| {
      let req = if s.required { "" } else { "?" };
      let default_note = match s.default {
        Some(CliOptionDefault::String(v)) => format!("={v}"),
        Some(CliOptionDefault::Bool(v)) => format!("={v}"),
        Some(CliOptionDefault::Usize(v)) => format!("={v}"),
        None => String::new(),
      };
      format!("(:{key}{req}{default_note})", key = s.key)
    })
    .collect();
  format!("({proc_name} $ {{}} {}) → {result_hint}", opts.join(" "))
}

pub fn schema_type_for_kind(kind: CliOptionKind) -> &'static str {
  match kind {
    CliOptionKind::String => ":string",
    CliOptionKind::Bool => ":bool",
    CliOptionKind::Usize => ":number",
    CliOptionKind::CirruQuote => ":cirru-quote",
    CliOptionKind::StringList => "(:: :list :string)",
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CORE_NS, CalcitList, CalcitProc, CalcitSyntax};
  use cirru_edn::EdnTag;
  use cirru_parser::Cirru;
  use rpds::HashTrieMap;

  fn map_with(pairs: &[(&str, Calcit)]) -> Calcit {
    let mut map = HashTrieMap::new_sync();
    for (k, v) in pairs {
      map.insert_mut(Calcit::Tag(EdnTag::new(*k)), v.clone());
    }
    Calcit::Map(map)
  }

  static PEEK_SPECS: &[CliOptionSpec] = &[
    cli_option("file-path", CliOptionKind::String, false, None),
    cli_option("target", CliOptionKind::String, true, None),
    cli_option("lines", CliOptionKind::Usize, false, Some(CliOptionDefault::Usize(5))),
  ];

  #[test]
  fn resolves_required_and_default() {
    let xs = vec![map_with(&[
      ("file-path", Calcit::Str(Arc::from("|a.cirru"))),
      ("target", Calcit::Str(Arc::from("|app.main/main!"))),
    ])];
    let args = resolve_cli_args("calcit.cli/peek-def", &xs, PEEK_SPECS).unwrap();
    assert_eq!(args.string("file-path").unwrap(), "a.cirru");
    assert_eq!(args.string("target").unwrap(), "app.main/main!");
    assert_eq!(args.usize("lines").unwrap(), 5);
    assert!(args.defaulted_keys().contains(&"lines"));
  }

  #[test]
  fn rejects_unknown_keys() {
    let xs = vec![map_with(&[
      ("oops", Calcit::Str(Arc::from("|x"))),
      ("file-path", Calcit::Str(Arc::from("|a.cirru"))),
    ])];
    let err = resolve_cli_args("calcit.cli/peek-def", &xs, PEEK_SPECS).unwrap_err();
    assert!(err.to_string().contains("unknown option"));
  }

  #[test]
  fn preprocess_check_catches_bad_cirru_quote_type() {
    let arg = map_with(&[
      ("file-path", Calcit::Str(Arc::from("|calcit/test.cirru"))),
      ("target", Calcit::Str(Arc::from("|app.main/main!"))),
      ("path", Calcit::Str(Arc::from("|3.2"))),
      ("code", Calcit::Str(Arc::from("|new-expr"))),
    ]);
    let issues = check_cli_options_map(
      "calcit.cli/tree-replace",
      &arg,
      &[
        cli_option("file-path", CliOptionKind::String, true, None),
        cli_option("target", CliOptionKind::String, true, None),
        cli_option("path", CliOptionKind::String, true, None),
        cli_option("code", CliOptionKind::CirruQuote, true, None),
      ],
    );
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "W_CLI_OPTION_TYPE_MISMATCH");
    assert!(issues[0].message.contains("cirru-quote"));
  }

  #[test]
  fn preprocess_check_accepts_quote_list_form() {
    let quote_list = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from(CORE_NS)),
      Calcit::Str(Arc::from("|new-expr")),
    ])));
    let arg = map_with(&[
      ("file-path", Calcit::Str(Arc::from("|calcit/test.cirru"))),
      ("target", Calcit::Str(Arc::from("|app.main/main!"))),
      ("path", Calcit::Str(Arc::from("|3.2"))),
      ("code", quote_list),
    ]);
    let issues = check_cli_options_map(
      "calcit.cli/tree-replace",
      &arg,
      &[
        cli_option("file-path", CliOptionKind::String, true, None),
        cli_option("target", CliOptionKind::String, true, None),
        cli_option("path", CliOptionKind::String, true, None),
        cli_option("code", CliOptionKind::CirruQuote, true, None),
      ],
    );
    assert!(issues.is_empty());
  }

  #[test]
  fn runtime_resolve_accepts_evaluated_quote_leaf() {
    static TREE_REPLACE_SPECS: &[CliOptionSpec] = &[
      cli_option("file-path", CliOptionKind::String, true, None),
      cli_option("target", CliOptionKind::String, true, None),
      cli_option("path", CliOptionKind::String, true, None),
      cli_option("code", CliOptionKind::CirruQuote, true, None),
    ];
    let xs = vec![map_with(&[
      ("file-path", Calcit::Str(Arc::from("|calcit/test.cirru"))),
      ("target", Calcit::Str(Arc::from("|app.main/main!"))),
      ("path", Calcit::Str(Arc::from("|3.2"))),
      ("code", Calcit::Str(Arc::from("|new-expr"))),
    ])];
    let args = resolve_cli_args("calcit.cli/tree-replace", &xs, TREE_REPLACE_SPECS).unwrap();
    assert_eq!(args.cirru_quote("code").unwrap(), Cirru::Leaf(Arc::from("|new-expr")));
  }

  #[test]
  fn preprocess_check_accepts_cirru_quote() {
    use cirru_parser::Cirru;
    let arg = map_with(&[
      ("file-path", Calcit::Str(Arc::from("|calcit/test.cirru"))),
      ("target", Calcit::Str(Arc::from("|app.main/main!"))),
      ("path", Calcit::Str(Arc::from("|3.2"))),
      ("code", Calcit::CirruQuote(Cirru::Leaf(Arc::from("new-expr")))),
    ]);
    let issues = check_cli_options_map(
      "calcit.cli/tree-replace",
      &arg,
      &[
        cli_option("file-path", CliOptionKind::String, true, None),
        cli_option("target", CliOptionKind::String, true, None),
        cli_option("path", CliOptionKind::String, true, None),
        cli_option("code", CliOptionKind::CirruQuote, true, None),
      ],
    );
    assert!(issues.is_empty());
  }

  #[test]
  fn preprocess_check_catches_bad_type() {
    let arg = map_with(&[
      ("file-path", Calcit::Str(Arc::from("|x"))),
      ("target", Calcit::Str(Arc::from("|y"))),
      ("lines", Calcit::Str(Arc::from("|bad"))),
    ]);
    let issues = check_cli_options_map("calcit.cli/peek-def", &arg, PEEK_SPECS);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "W_CLI_OPTION_TYPE_MISMATCH");
  }

  #[test]
  fn preprocess_check_catches_typo_key() {
    let arg = map_with(&[
      ("file-pth", Calcit::Str(Arc::from("|x"))),
      ("file-path", Calcit::Str(Arc::from("|calcit/test.cirru"))),
    ]);
    let issues = check_cli_options_map(
      "calcit.cli/list-ns",
      &arg,
      &[cli_option("file-path", CliOptionKind::String, true, None)],
    );
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "W_CLI_OPTION_UNKNOWN_KEY");
  }

  #[test]
  fn preprocess_check_catches_missing_required() {
    let arg = map_with(&[("file-path", Calcit::Str(Arc::from("|calcit/test.cirru")))]);
    let issues = check_cli_options_map(
      "calcit.cli/peek-def",
      &arg,
      &[
        cli_option("file-path", CliOptionKind::String, false, None),
        cli_option("target", CliOptionKind::String, true, None),
        cli_option("lines", CliOptionKind::Usize, false, Some(CliOptionDefault::Usize(5))),
      ],
    );
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "W_CLI_OPTION_MISSING_REQUIRED");
  }

  #[test]
  fn preprocess_check_accepts_string_list_paths() {
    static TREE_BATCH_DELETE_SPECS: &[CliOptionSpec] = &[
      cli_option("file-path", CliOptionKind::String, false, None),
      cli_option("target", CliOptionKind::String, true, None),
      cli_option("paths", CliOptionKind::StringList, true, None),
    ];
    let paths = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Proc(CalcitProc::List),
      Calcit::Str(Arc::from("|3.2")),
      Calcit::Str(Arc::from("|4.1")),
    ])));
    let arg = map_with(&[("target", Calcit::Str(Arc::from("|app.main/main!"))), ("paths", paths)]);
    let issues = check_cli_options_map("calcit.cli/tree-batch-delete", &arg, TREE_BATCH_DELETE_SPECS);
    assert!(issues.is_empty());
  }

  #[test]
  fn preprocess_check_rejects_csv_paths_string() {
    static TREE_BATCH_DELETE_SPECS: &[CliOptionSpec] = &[
      cli_option("target", CliOptionKind::String, true, None),
      cli_option("paths", CliOptionKind::StringList, true, None),
    ];
    let arg = map_with(&[
      ("target", Calcit::Str(Arc::from("|app.main/main!"))),
      ("paths", Calcit::Str(Arc::from("|3.2,4.1"))),
    ]);
    let issues = check_cli_options_map("calcit.cli/tree-batch-delete", &arg, TREE_BATCH_DELETE_SPECS);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "W_CLI_OPTION_TYPE_MISMATCH");
  }

  #[test]
  fn file_path_falls_back_to_host_snapshot() {
    static PEEK_SPECS_OPTIONAL_FILE: &[CliOptionSpec] = &[
      cli_option("file-path", CliOptionKind::String, false, None),
      cli_option("target", CliOptionKind::String, true, None),
      cli_option("lines", CliOptionKind::Usize, false, Some(CliOptionDefault::Usize(5))),
    ];
    crate::set_host_snapshot_file(Some("calcit/test.cirru".to_string()));
    let xs = vec![map_with(&[("target", Calcit::Str(Arc::from("|app.main/main!")))])];
    let args = resolve_cli_args("calcit.cli/peek-def", &xs, PEEK_SPECS_OPTIONAL_FILE).unwrap();
    assert_eq!(args.file_path().unwrap(), "calcit/test.cirru");
    crate::set_host_snapshot_file(None);
  }

  static LIST_NS_ONE_ARG: &[CliOptionSpec] = &[cli_option("file-path", CliOptionKind::String, false, None)];

  #[test]
  fn rejects_positional_args() {
    let xs = vec![Calcit::Str(Arc::from("|a.cirru"))];
    let err = resolve_cli_args("calcit.cli/list-ns", &xs, LIST_NS_ONE_ARG).unwrap_err();
    assert!(err.to_string().contains("expected options map"));
  }
}
