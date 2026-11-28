//! Read-only static inventory of 0.47.0 legacy nil, dynamic-style, and runtime-predicate patterns (#2973).

use super::{QuoteContext, WeakTypeContext, format_cirru_path, scoped_definition_entries};
use crate::cli_handlers;
use calcit::cli_args::WeakTypesCommand;
use calcit::snapshot;
use cirru_parser::Cirru;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MigrationCategory {
  LegacyNil,
  LegacyDynamicStyle,
  RuntimeTypePredicate,
}

impl MigrationCategory {
  fn as_str(self) -> &'static str {
    match self {
      Self::LegacyNil => "legacy-nil",
      Self::LegacyDynamicStyle => "legacy-dynamic-style",
      Self::RuntimeTypePredicate => "runtime-type-predicate",
    }
  }

  fn diagnostic_code(self) -> &'static str {
    match self {
      Self::LegacyNil => snapshot::LEGACY_NIL_USAGE_ERROR,
      Self::LegacyDynamicStyle => snapshot::LEGACY_DYNAMIC_STYLE_ERROR,
      Self::RuntimeTypePredicate => snapshot::RUNTIME_TYPE_PREDICATE_WARNING,
    }
  }

  fn default_severity(self) -> &'static str {
    match self {
      Self::LegacyNil | Self::LegacyDynamicStyle => "error",
      Self::RuntimeTypePredicate => "warning",
    }
  }
}

struct LegacyFinding<'a> {
  category: MigrationCategory,
  symbol: &'static str,
  path: Vec<usize>,
  snippet: &'a Cirru,
  hint: &'static str,
}

fn leaf(node: &Cirru) -> Option<&str> {
  match node {
    Cirru::Leaf(text) => Some(text.as_ref()),
    Cirru::List(_) => None,
  }
}

fn classify_core_call(symbol: &str) -> Option<(MigrationCategory, &'static str, &'static str)> {
  match symbol {
    ";nil" => Some((
      MigrationCategory::LegacyNil,
      ";nil",
      "Removed in 0.48.0. Replace with `&unit` (no-value statement; auto-fixable for empty `(;nil)` in discarded/Unit-tail positions via `calcit fix --rule legacy-comment-nil-v1`) or `Option :none` (absence).",
    )),
    "either" => Some((
      MigrationCategory::LegacyNil,
      "either",
      "Removed in 0.48.0. Migrate to `Option<T>` with `.unwrap-or`, `if-let`, or `match`.",
    )),
    "non-nil!" => Some((
      MigrationCategory::LegacyNil,
      "non-nil!",
      "Removed in 0.48.0. Migrate to `Option<T>` with `match`, `if-let`, `.unwrap-or`, or `.unwrap`.",
    )),
    "keys-non-nil" => Some((
      MigrationCategory::LegacyNil,
      "keys-non-nil",
      "Removed in 0.48.0. Store `Option<V>` or omit absent keys instead of treating nil map values as absent.",
    )),
    "merge-non-nil" => Some((
      MigrationCategory::LegacyNil,
      "merge-non-nil",
      "Removed in 0.48.0. Store `Option<V>` or omit absent keys instead of treating nil map values as absent.",
    )),
    "&merge-non-nil" => Some((
      MigrationCategory::LegacyNil,
      "&merge-non-nil",
      "Removed in 0.48.0. Store `Option<V>` or omit absent keys instead of treating nil map values as absent.",
    )),
    "impl-traits" => Some((
      MigrationCategory::LegacyDynamicStyle,
      "impl-traits",
      "Declare static top-level `(:impls …)` + `defimpl`, or use `(Dyn Trait)` (`dyn:box`) for dynamic polymorphism.",
    )),
    "&struct:impl-traits" => Some((
      MigrationCategory::LegacyDynamicStyle,
      "&struct:impl-traits",
      "Declare static top-level `(:impls …)` + `defimpl`, or use `(Dyn Trait)` (`dyn:box`) for dynamic polymorphism.",
    )),
    "&enum:impl-traits" => Some((
      MigrationCategory::LegacyDynamicStyle,
      "&enum:impl-traits",
      "Declare static top-level `(:impls …)` + `defimpl`, or use `(Dyn Trait)` (`dyn:box`) for dynamic polymorphism.",
    )),
    "&struct-def:impl-traits" => Some((
      MigrationCategory::LegacyDynamicStyle,
      "&struct-def:impl-traits",
      "Declare static top-level `(:impls …)` + `defimpl`, or use `(Dyn Trait)` (`dyn:box`) for dynamic polymorphism.",
    )),
    "&enum-def:impl-traits" => Some((
      MigrationCategory::LegacyDynamicStyle,
      "&enum-def:impl-traits",
      "Declare static top-level `(:impls …)` + `defimpl`, or use `(Dyn Trait)` (`dyn:box`) for dynamic polymorphism.",
    )),
    "%::" => Some((
      MigrationCategory::LegacyDynamicStyle,
      "%::",
      "Use a closed nominal `defenum` direct constructor (`Enum :variant …`, or `calcit fix --rule named-enum-constructor-v1`) or `::` for anonymous enums.",
    )),
    "nil?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "nil?",
      "Migrate to `Option<T>` (`match`, `if-let`, `.some?`/`.none?`) or `js-nullish?` at JS FFI boundaries; on concrete non-nullable types this is a blocking `E_LEGACY_NIL_USAGE` error.",
    )),
    "non-nil?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "non-nil?",
      "Migrate to `Option<T>` (`match`, `if-let`, `.some?`/`.none?`) or `js-nullish?` at JS FFI boundaries; on concrete non-nullable types this is a blocking `E_LEGACY_NIL_USAGE` error.",
    )),
    "type-of" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "type-of",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "data-view" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "data-view",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "bool?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "bool?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "number?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "number?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "string?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "string?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "tag?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "tag?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "symbol?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "symbol?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "list?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "list?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "map?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "map?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "set?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "set?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "fn?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "fn?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "struct?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "struct?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "enum?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "enum?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "tuple?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "tuple?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "buffer?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "buffer?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "buf-list?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "buf-list?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "ref?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "ref?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "struct-def?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "struct-def?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    "enum-def?" => Some((
      MigrationCategory::RuntimeTypePredicate,
      "enum-def?",
      "Prefer closed nominal `defenum`/`defstruct`/`EdnValue`/`JsonValue` with `match`, or `Any` (`any:downcast`) / `(Dyn Trait)`.",
    )),
    _ => None,
  }
}

fn is_callable_core_value_ref(symbol: &str) -> bool {
  matches!(
    symbol,
    "either"
      | "non-nil!"
      | "keys-non-nil"
      | "merge-non-nil"
      | "&merge-non-nil"
      | "nil?"
      | "non-nil?"
      | "type-of"
      | "data-view"
      | "bool?"
      | "number?"
      | "string?"
      | "tag?"
      | "symbol?"
      | "list?"
      | "map?"
      | "set?"
      | "fn?"
      | "struct?"
      | "enum?"
      | "tuple?"
      | "buffer?"
      | "buf-list?"
      | "ref?"
      | "struct-def?"
      | "enum-def?"
  )
}

fn resolve_core_symbol<'a>(raw: &'a str, shadowed: &HashSet<String>) -> Option<&'a str> {
  if let Some(qualified) = raw.strip_prefix("calcit.core/") {
    return Some(qualified);
  }
  if raw.contains('/') || shadowed.contains(raw) {
    None
  } else {
    Some(raw)
  }
}

fn collect_binding_names(node: &Cirru, target: &mut HashSet<String>) {
  match node {
    Cirru::Leaf(name) => {
      let name = name.as_ref();
      if !matches!(name, "&" | "?" | "[]" | "{}") && !name.starts_with([':', '|', '"', '\'']) {
        target.insert(name.to_owned());
      }
    }
    Cirru::List(items) => {
      for item in items {
        collect_binding_names(item, target);
      }
    }
  }
}

fn collect<'a>(
  node: &'a Cirru,
  path: &mut Vec<usize>,
  quote: QuoteContext,
  type_context: WeakTypeContext,
  shadowed: &HashSet<String>,
  findings: &mut Vec<LegacyFinding<'a>>,
) {
  match node {
    Cirru::Leaf(text) => {
      if !quote.is_quoted()
        && type_context == WeakTypeContext::Code
        && let Some(core_symbol) = resolve_core_symbol(text.as_ref(), shadowed)
        && is_callable_core_value_ref(core_symbol)
        && let Some((category, canonical, hint)) = classify_core_call(core_symbol)
      {
        findings.push(LegacyFinding {
          category,
          symbol: canonical,
          path: path.clone(),
          snippet: node,
          hint,
        });
      }
    }
    Cirru::List(items) => {
      let head = items.first().and_then(leaf);
      let core_head = head.and_then(|name| resolve_core_symbol(name, shadowed));

      if matches!(core_head, Some(";" | "cirru-quote")) && !quote.is_quoted() {
        return;
      }
      if core_head == Some(";") {
        return;
      }

      if !quote.is_quoted() && type_context == WeakTypeContext::Code {
        if core_head == Some(";nil") {
          let (_, canonical, hint) = classify_core_call(";nil").expect(";nil classification");
          findings.push(LegacyFinding {
            category: MigrationCategory::LegacyNil,
            symbol: canonical,
            path: path.clone(),
            snippet: node,
            hint,
          });
          // `;nil` ignores all argument forms at expansion time.
          return;
        }
        if core_head == Some("%{}") && items.get(1).and_then(leaf) == Some("_") {
          findings.push(LegacyFinding {
            category: MigrationCategory::LegacyDynamicStyle,
            symbol: "%{} _",
            path: path.clone(),
            snippet: node,
            hint: "Use a closed nominal `defstruct` with direct constructors (`Struct :field value`).",
          });
        } else if let Some(core_name) = core_head
          && let Some((category, canonical, hint)) = classify_core_call(core_name)
        {
          findings.push(LegacyFinding {
            category,
            symbol: canonical,
            path: path.clone(),
            snippet: node,
            hint,
          });
        }
      }

      let mut inner_shadowed = CowShadow::Borrowed(shadowed);
      if !quote.is_quoted() && type_context == WeakTypeContext::Code {
        match core_head {
          Some("defn" | "defmacro") => {
            if let Some(params) = items.get(2) {
              collect_binding_names(params, inner_shadowed.to_mut());
            }
          }
          Some("fn") => {
            if let Some(params) = items.get(1) {
              collect_binding_names(params, inner_shadowed.to_mut());
            }
          }
          Some("let") => {
            if let Some(Cirru::List(bindings)) = items.get(1) {
              for (bind_idx, binding) in bindings.iter().enumerate() {
                path.push(1);
                path.push(bind_idx);
                if let Cirru::List(pair) = binding
                  && let Some(value) = pair.get(1)
                {
                  path.push(1);
                  collect(
                    value,
                    path,
                    quote.for_child(core_head, 1),
                    type_context.for_child(head, 1, items.len()),
                    inner_shadowed.as_ref(),
                    findings,
                  );
                  path.pop();
                }
                path.pop();
                path.pop();
                if let Cirru::List(pair) = binding
                  && let Some(binder) = pair.first()
                {
                  collect_binding_names(binder, inner_shadowed.to_mut());
                }
              }
            }
            for (index, child) in items.iter().enumerate().skip(2) {
              path.push(index);
              collect(
                child,
                path,
                quote.for_child(core_head, index),
                type_context.for_child(head, index, items.len()),
                inner_shadowed.as_ref(),
                findings,
              );
              path.pop();
            }
            return;
          }
          Some("&let") => {
            if let Some(Cirru::List(pair)) = items.get(1) {
              if let Some(value) = pair.get(1) {
                path.push(1);
                path.push(1);
                collect(
                  value,
                  path,
                  quote.for_child(core_head, 1),
                  type_context.for_child(head, 1, items.len()),
                  inner_shadowed.as_ref(),
                  findings,
                );
                path.pop();
                path.pop();
              }
              if let Some(binder) = pair.first() {
                collect_binding_names(binder, inner_shadowed.to_mut());
              }
            }
            for (index, child) in items.iter().enumerate().skip(2) {
              path.push(index);
              collect(
                child,
                path,
                quote.for_child(core_head, index),
                type_context.for_child(head, index, items.len()),
                inner_shadowed.as_ref(),
                findings,
              );
              path.pop();
            }
            return;
          }
          Some("let[]" | "let{}") => {
            if let Some(value) = items.get(2) {
              path.push(2);
              collect(
                value,
                path,
                quote.for_child(core_head, 2),
                type_context.for_child(head, 2, items.len()),
                inner_shadowed.as_ref(),
                findings,
              );
              path.pop();
            }
            if let Some(binding_pattern) = items.get(1) {
              collect_binding_names(binding_pattern, inner_shadowed.to_mut());
            }
            for (index, child) in items.iter().enumerate().skip(3) {
              path.push(index);
              collect(
                child,
                path,
                quote.for_child(core_head, index),
                type_context.for_child(head, index, items.len()),
                inner_shadowed.as_ref(),
                findings,
              );
              path.pop();
            }
            return;
          }
          Some("match") => {
            if let Some(scrutinee) = items.get(1) {
              path.push(1);
              collect(
                scrutinee,
                path,
                quote.for_child(core_head, 1),
                type_context.for_child(head, 1, items.len()),
                inner_shadowed.as_ref(),
                findings,
              );
              path.pop();
            }
            for (clause_idx, clause) in items.iter().enumerate().skip(2) {
              let Cirru::List(clause_items) = clause else {
                continue;
              };
              let mut clause_shadowed = inner_shadowed.as_ref().clone();
              if let Some(pattern) = clause_items.first() {
                collect_binding_names(pattern, &mut clause_shadowed);
              }
              path.push(clause_idx);
              for (expr_idx, expr) in clause_items.iter().enumerate().skip(1) {
                path.push(expr_idx);
                collect(
                  expr,
                  path,
                  quote.for_child(core_head, clause_idx),
                  WeakTypeContext::Code,
                  &clause_shadowed,
                  findings,
                );
                path.pop();
              }
              path.pop();
            }
            return;
          }
          _ => {}
        }
      }

      for (index, child) in items.iter().enumerate() {
        // Avoid double-counting a leaf already classified as the call head.
        if index == 0 && matches!(child, Cirru::Leaf(_)) && !quote.is_quoted() && type_context == WeakTypeContext::Code {
          continue;
        }
        // Skip definition-name slots and parameter declaration list children in definitions.
        let is_definition_name = index == 1
          && matches!(
            core_head,
            Some(
              "defn"
                | "defmacro"
                | "def"
                | "defstruct"
                | "defenum"
                | "deftrait"
                | "deftype"
                | "defexternal"
                | "defatom"
                | "defref"
                | "defwasm-export"
                | "defwasm-import"
            )
          );
        let is_param_list = (matches!(core_head, Some("defn" | "defmacro")) && index == 2) || (core_head == Some("fn") && index == 1);
        if !quote.is_quoted() && type_context == WeakTypeContext::Code && (is_definition_name || is_param_list) {
          continue;
        }
        path.push(index);
        collect(
          child,
          path,
          quote.for_child(core_head, index),
          type_context.for_child(head, index, items.len()),
          inner_shadowed.as_ref(),
          findings,
        );
        path.pop();
      }
    }
  }
}

enum CowShadow<'a> {
  Borrowed(&'a HashSet<String>),
  Owned(HashSet<String>),
}

impl CowShadow<'_> {
  fn as_ref(&self) -> &HashSet<String> {
    match self {
      Self::Borrowed(set) => set,
      Self::Owned(set) => set,
    }
  }

  fn to_mut(&mut self) -> &mut HashSet<String> {
    if let Self::Borrowed(set) = self {
      *self = Self::Owned((*set).clone());
    }
    match self {
      Self::Owned(set) => set,
      Self::Borrowed(_) => unreachable!("converted to owned"),
    }
  }
}

pub(crate) fn run(options: &WeakTypesCommand, snapshot: &snapshot::Snapshot) -> Result<(), String> {
  if !matches!(options.format.as_str(), "human" | "text" | "edn" | "json") {
    return Err(format!(
      "Unknown weak-types output format `{}`. Expected `human`, `edn`, or `json`.",
      options.format
    ));
  }
  let entries = scoped_definition_entries(snapshot, options.ns.as_deref(), options.ns_prefix.as_deref(), options.deps)?;
  let mut findings = Vec::new();
  let mut definition_hits = BTreeMap::<String, usize>::new();
  let mut by_category_map = BTreeMap::from([
    (MigrationCategory::LegacyNil.as_str(), 0usize),
    (MigrationCategory::LegacyDynamicStyle.as_str(), 0usize),
    (MigrationCategory::RuntimeTypePredicate.as_str(), 0usize),
  ]);
  let mut by_symbol = BTreeMap::<&'static str, usize>::new();

  for (namespace, definition, entry) in entries {
    let mut local_findings = Vec::new();
    let shadowed = HashSet::new();
    collect(
      &entry.code,
      &mut Vec::new(),
      QuoteContext::default(),
      WeakTypeContext::Code,
      &shadowed,
      &mut local_findings,
    );
    if local_findings.is_empty() {
      continue;
    }
    let full_def = format!("{namespace}/{definition}");
    *definition_hits.entry(full_def.clone()).or_insert(0) += local_findings.len();
    for finding in local_findings {
      *by_category_map.entry(finding.category.as_str()).or_insert(0) += 1;
      *by_symbol.entry(finding.symbol).or_insert(0) += 1;
      findings.push(json!({
        "definition": full_def,
        "path": format_cirru_path("code", &finding.path),
        "category": finding.category.as_str(),
        "diagnostic_code": finding.category.diagnostic_code(),
        "severity": finding.category.default_severity(),
        "symbol": finding.symbol,
        "expression": cli_handlers::format_cirru_expression(finding.snippet),
        "hint": finding.hint,
      }));
    }
  }

  let by_category_json = json!({
    "legacy_nil": by_category_map["legacy-nil"],
    "legacy_dynamic_style": by_category_map["legacy-dynamic-style"],
    "runtime_type_predicate": by_category_map["runtime-type-predicate"],
  });
  let by_symbol_json = by_symbol
    .iter()
    .map(|(symbol, count)| json!({ "symbol": symbol, "count": count }))
    .collect::<Vec<_>>();

  let report = json!({
    "schema_version": 1,
    "command": "analyze.weak-types",
    "data": {
      "view": cli_handlers::VIEW_LEGACY_MIGRATION,
      "filters": { "ns": options.ns, "ns_prefix": options.ns_prefix, "deps": options.deps },
      "summary": {
        "findings": findings.len(),
        "definitions": definition_hits.len(),
        "by_category": by_category_json,
        "by_symbol": by_symbol_json,
      },
      "findings": if options.summary_only { Vec::<Value>::new() } else { findings.clone() },
    },
    "diagnostics": [],
  });

  match options.format.as_str() {
    "edn" => println!("{}", cli_handlers::format_json_value_as_edn(&report)?),
    "json" => println!("{}", serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?),
    _ => {
      println!(
        "# Legacy Migration Inventory\n\n{} finding(s) across {} definition(s) (legacy-nil={}, legacy-dynamic-style={}, runtime-type-predicate={}).",
        findings.len(),
        definition_hits.len(),
        by_category_map["legacy-nil"],
        by_category_map["legacy-dynamic-style"],
        by_category_map["runtime-type-predicate"],
      );
      if !by_symbol.is_empty() {
        let breakdown = by_symbol
          .iter()
          .map(|(symbol, count)| format!("{symbol}={count}"))
          .collect::<Vec<_>>()
          .join(", ");
        println!("- Symbols: {breakdown}");
      }
      if !options.summary_only {
        for finding in &findings {
          println!(
            "\n## {} @ {}\n\n- Category: `{}` (`{}`)\n- Symbol: `{}`\n",
            finding["definition"].as_str().unwrap(),
            finding["path"].as_str().unwrap(),
            finding["category"].as_str().unwrap(),
            finding["diagnostic_code"].as_str().unwrap(),
            finding["symbol"].as_str().unwrap(),
          );
          print!(
            "{}",
            cli_handlers::markdown_fenced_block("cirru", finding["expression"].as_str().unwrap())
          );
          println!("\n- Hint: {}", finding["hint"].as_str().unwrap());
        }
      }
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn classifies_all_three_categories_and_respects_quotes_comments_and_shadowing() {
    let source = cirru_parser::parse(
      r#"
defn demo (x either)
  ; $ either nil 1
  ;nil (non-nil! x)
  let ((local (either x 0)) (anon (%{} _ (:a 1))) (named (%{} Point (:x 1))) (e (%:: Either :left 1)))
    if (tag? local)
      first $ filter local non-nil?
      assoc-in nil [] :ok
  impl-traits x Show
  quote $ nil? x
  quasiquote $ [] (type-of x) (~ $ bool? x)
"#,
    )
    .unwrap();
    let mut findings = Vec::new();
    collect(
      &source[0],
      &mut Vec::new(),
      QuoteContext::default(),
      WeakTypeContext::Code,
      &HashSet::new(),
      &mut findings,
    );
    let summary: Vec<(&str, &str)> = findings.iter().map(|f| (f.category.as_str(), f.symbol)).collect();
    // `either` is shadowed by param so `(either x 0)` is NOT counted; `;nil`'s arg `(non-nil! x)` is skipped;
    // `%{} Point` is NOT `%{} _`; `quote` & un-unquoted `quasiquote` are skipped, while `~ (bool? x)` IS counted.
    assert_eq!(
      summary,
      vec![
        ("legacy-nil", ";nil"),
        ("legacy-dynamic-style", "%{} _"),
        ("legacy-dynamic-style", "%::"),
        ("runtime-type-predicate", "tag?"),
        ("runtime-type-predicate", "non-nil?"),
        ("legacy-dynamic-style", "impl-traits"),
        ("runtime-type-predicate", "bool?"),
      ]
    );
    // Ensure inline helper wasn't needed and paths are populated.
    assert!(!findings.is_empty());

    let edge_source = cirru_parser::parse(
      r#"
do
  defn tag? (x) true
  &let (tag? (fn (x) true)) (tag? 1)
  cirru-quote (nil? x)
"#,
    )
    .unwrap();
    let mut edge_findings = Vec::new();
    collect(
      &edge_source[0],
      &mut Vec::new(),
      QuoteContext::default(),
      WeakTypeContext::Code,
      &HashSet::new(),
      &mut edge_findings,
    );
    assert!(
      edge_findings.is_empty(),
      "definition names, &let shadowers, and cirru-quote payloads must not trigger false positives"
    );
  }
}
