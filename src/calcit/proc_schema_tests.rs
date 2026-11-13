use std::{collections::BTreeSet, sync::Arc};

use strum::IntoEnumIterator;

use super::{CalcitProc, CalcitTypeAnnotation, type_refs::map_type_references};

#[test]
fn proc_schema_differences_do_not_grow() {
  let snapshot = crate::load_core_snapshot().expect("load generated core snapshot");
  let core = &snapshot.files["calcit.core"];

  let qualify = |annotation: &Arc<CalcitTypeAnnotation>| {
    map_type_references(annotation.clone(), &|name, args| {
      let name = if core.defs.contains_key(name.as_ref()) {
        Arc::from(format!("calcit.core/{name}"))
      } else {
        name.clone()
      };
      Arc::new(CalcitTypeAnnotation::TypeRef(name, args))
    })
  };

  let mut differences = BTreeSet::new();
  for proc in CalcitProc::iter() {
    let Some(signature) = proc.get_type_signature() else {
      continue;
    };
    let Some(entry) = core.defs.get(proc.as_ref()) else {
      differences.insert(format!("{proc} | missing core definition"));
      continue;
    };
    let Some(schema) = entry.schema.as_function() else {
      differences.insert(format!("{proc} | missing core function schema"));
      continue;
    };

    // Compare fixed parameters and rest separately, regardless of how each
    // representation stores the rest element type.
    let rust = CalcitTypeAnnotation::from_function_parts(signature.arg_types.clone(), signature.return_type.clone());
    let rust = rust.as_function().expect("Rust proc function");
    let mut compare = |position: &str, rust: Option<&Arc<CalcitTypeAnnotation>>, core: Option<&Arc<CalcitTypeAnnotation>>| {
      let rust = rust.map(&qualify);
      let core = core.map(&qualify);
      if rust != core {
        let render =
          |ty: Option<Arc<CalcitTypeAnnotation>>| ty.map_or_else(|| "<missing>".to_owned(), |ty| ty.to_type_edn().cirru().to_string());
        differences.insert(format!("{proc} | {position} | {} => {}", render(rust), render(core)));
      }
    };
    for index in 0..rust.arg_types.len().max(schema.arg_types.len()) {
      compare(&format!("arg {index}"), rust.arg_types.get(index), schema.arg_types.get(index));
    }
    compare("rest", rust.rest_type.as_ref(), schema.rest_type.as_ref());
    compare("return", Some(&rust.return_type), Some(&schema.return_type));
  }

  let baseline = include_str!("proc-schema-differences.txt")
    .lines()
    .filter(|line| !line.is_empty() && !line.starts_with('#'))
    .collect::<BTreeSet<_>>();
  let added = differences
    .iter()
    .filter(|line| !baseline.contains(line.as_str()))
    .map(String::as_str)
    .collect::<Vec<_>>();
  assert!(added.is_empty(), "new Rust proc / core schema differences:\n{}", added.join("\n"));
}
