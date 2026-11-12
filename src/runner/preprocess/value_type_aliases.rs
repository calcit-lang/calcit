//! `W_VALUE_TYPE_ALIAS`: a type position that names a plain value definition.
//!
//! Before `deftype`, a type alias was written as a value `def` whose schema
//! carried the type (`def Vec3 ([] 0 0 0)` with `:schema (Tuple Number ...)`,
//! or a `defn` whose `Fn` schema named a callback shape).
//! That spelling still resolves, but this advisory points at the `deftype`
//! form that replaces it.

use std::sync::Arc;

use crate::calcit::{Calcit, CalcitTypeAnnotation, LocatedWarning, NodeLocation};
use crate::program;

use super::{GENERATED_DEF, push_advisory, should_emit_project_source_lint};

pub(crate) const VALUE_TYPE_ALIAS_WARNING: &str = "W_VALUE_TYPE_ALIAS";

/// The `(ns, def, schema)` of a value definition used as a type alias.
fn value_alias_target(
  file_ns: &str,
  annotation: &Arc<CalcitTypeAnnotation>,
) -> Option<(Arc<str>, Arc<str>, Arc<CalcitTypeAnnotation>)> {
  let CalcitTypeAnnotation::TypeRef(name, _) = annotation.as_ref() else {
    return None;
  };
  let (ns, def) = program::resolve_type_name_in_ns(file_ns, name)?;
  let code = program::lookup_def_code(&ns, &def)?;
  let Calcit::List(items) = &code else { return None };
  if !matches!(items.first(), Some(Calcit::Symbol { sym, .. }) if matches!(sym.as_ref(), "def" | "defn")) {
    return None;
  }
  let schema = program::lookup_def_schema(&ns, &def);
  // Nominal re-exports (`def PointAlias Point`) and open schemas are not aliases.
  match schema.as_ref() {
    CalcitTypeAnnotation::Dynamic
    | CalcitTypeAnnotation::Custom(_)
    | CalcitTypeAnnotation::StructDef(_)
    | CalcitTypeAnnotation::EnumDef(_)
    | CalcitTypeAnnotation::Trait(_) => None,
    _ => Some((ns, def, schema)),
  }
}

/// Note the first value-style alias in `annotation`, with the `deftype` that replaces it.
pub(super) fn note_value_type_aliases(
  owner: &str,
  position: &str,
  annotation: &Arc<CalcitTypeAnnotation>,
  file_ns: &str,
  location: Option<NodeLocation>,
) {
  if !should_emit_project_source_lint(file_ns) {
    return;
  }
  let found = super::find_schema_annotation(annotation, "root", &mut |candidate, _| value_alias_target(file_ns, candidate));
  if let Some((ns, def, schema)) = found {
    let location = location.unwrap_or_else(|| NodeLocation::new(Arc::from(file_ns), Arc::from(GENERATED_DEF), Arc::new(vec![])));
    push_advisory(LocatedWarning::new_with_detail(
      format!(
        "[Warn] {owner} uses value definition `{ns}/{def}` as a type in {position}; declare it as `deftype {def} $ {}` instead, value-style aliases will be retired",
        compact_type_text(&schema)
      ),
      location,
      Some(VALUE_TYPE_ALIAS_WARNING.to_owned()),
      None,
    ));
  }
}

/// The schema in compact type syntax, falling back to the brief form.
fn compact_type_text(schema: &CalcitTypeAnnotation) -> String {
  crate::calcit::type_expr::print_type_compact(&schema.to_type_edn())
    .ok()
    .and_then(|form| cirru_parser::format_expr_one_liner(&form).ok())
    .unwrap_or_else(|| schema.to_brief_string())
}
