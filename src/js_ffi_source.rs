//! Definition-level JavaScript implementations shared by inline and file assets.

use crate::calcit::CalcitTypeAnnotation;
use crate::snapshot::{SnapshotTarget, ffi_metadata_value, parse_ffi_target};
use cirru_edn::Edn;
use std::path::{Component, Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsFfiSource {
  Inline(String),
  File { path: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsFfiImplementation {
  pub source: JsFfiSource,
  pub modules: Vec<(String, String)>,
}

fn field<'a>(value: &'a Edn, name: &str) -> Option<&'a Edn> {
  ffi_metadata_value(value, name)
}

fn string_field(value: &Edn, name: &str) -> Result<Option<String>, String> {
  match field(value, name) {
    None => Ok(None),
    Some(Edn::Str(value)) => Ok(Some(value.to_string())),
    Some(other) => Err(format!("`:js :{name}` must be a string, got `{other}`")),
  }
}

pub fn parse_js_source(ffi: &Edn) -> Result<Option<JsFfiImplementation>, String> {
  let Some(js) = field(ffi, "js") else { return Ok(None) };
  if !matches!(js, Edn::Map(_) | Edn::Struct(_)) {
    return Err(format!("`:ffi :js` must be a map, got `{js}`"));
  }
  let inline = string_field(js, "inline")?;
  let file = string_field(js, "file")?;
  if field(js, "export").is_some() || field(js, "assets").is_some() {
    return Err("`:js :export` and `:assets` are unsupported; a JS file must contain one expression".to_owned());
  }
  let mut modules = match field(js, "modules") {
    None => vec![],
    Some(Edn::Map(entries)) => entries
      .0
      .iter()
      .map(|(alias, specifier)| {
        let Edn::Tag(alias) = alias else {
          return Err("`:js :modules` aliases must be tags".to_owned());
        };
        let Edn::Str(specifier) = specifier else {
          return Err("`:js :modules` specifiers must be strings".to_owned());
        };
        if !is_js_identifier(alias.ref_str())
          || matches!(
            alias.ref_str(),
            "await"
              | "break"
              | "case"
              | "catch"
              | "class"
              | "const"
              | "continue"
              | "debugger"
              | "default"
              | "delete"
              | "do"
              | "else"
              | "export"
              | "extends"
              | "false"
              | "finally"
              | "for"
              | "function"
              | "if"
              | "import"
              | "in"
              | "instanceof"
              | "let"
              | "new"
              | "null"
              | "return"
              | "super"
              | "switch"
              | "this"
              | "throw"
              | "true"
              | "try"
              | "typeof"
              | "var"
              | "void"
              | "while"
              | "with"
              | "yield"
          )
        {
          return Err(format!("invalid JS module alias `{alias}`"));
        }
        if !valid_module_specifier(specifier) {
          return Err(format!("JS module `{specifier}` must be a node: builtin or bare package specifier"));
        }
        Ok((alias.ref_str().to_owned(), specifier.to_string()))
      })
      .collect::<Result<Vec<_>, String>>()?,
    Some(_) => return Err("`:js :modules` must be a map of aliases to module specifiers".to_owned()),
  };
  modules.sort();
  let source = match (inline, file) {
    (Some(source), None) if !source.trim().is_empty() => JsFfiSource::Inline(source),
    (None, Some(path)) => {
      validate_resource_path(&path)?;
      JsFfiSource::File { path }
    }
    _ => return Err("`:ffi :js` requires exactly one of nonempty `:inline` or `:file`".to_owned()),
  };
  Ok(Some(JsFfiImplementation { source, modules }))
}

pub fn validate_js_source(ffi: &Edn, schema: &CalcitTypeAnnotation) -> Result<Option<JsFfiImplementation>, String> {
  let Some(source) = parse_js_source(ffi)? else { return Ok(None) };
  match parse_ffi_target(ffi)? {
    Some(SnapshotTarget::Browser) if source.modules.iter().any(|(_, module)| module.starts_with("node:")) => {
      return Err("`:js :modules` node: imports require the node target".to_owned());
    }
    Some(SnapshotTarget::Browser | SnapshotTarget::Node) => {}
    None => return Err("`:ffi :js` requires an explicit browser or node target".to_owned()),
    Some(target) => return Err(format!("`:ffi :js` cannot target {target:?}; use browser or node")),
  }
  let CalcitTypeAnnotation::Fn(signature) = schema else {
    return Err("`:ffi :js` requires an explicit Fn schema".to_owned());
  };
  if !signature.features.iter().any(|feature| feature.ref_str() == "js-ffi") {
    return Err("`:ffi :js` requires `:js-ffi` in the Fn schema features".to_owned());
  }
  if !signature.generics.is_empty() || signature.rest_type.is_some() || signature.is_async_invocation() {
    return Err("`:ffi :js` does not yet support generic, rest, or async signatures".to_owned());
  }
  for (index, argument) in signature.arg_types.iter().enumerate() {
    if !supported_abi_type(argument) {
      return Err(format!(
        "`:ffi :js` argument {} has unsupported ABI type `{}`",
        index + 1,
        argument.to_brief_string()
      ));
    }
  }
  if !supported_abi_type(&signature.return_type) {
    return Err(format!(
      "`:ffi :js` return has unsupported ABI type `{}`",
      signature.return_type.to_brief_string()
    ));
  }
  Ok(Some(source))
}

fn supported_abi_type(value: &CalcitTypeAnnotation) -> bool {
  match value {
    CalcitTypeAnnotation::Bool
    | CalcitTypeAnnotation::Number
    | CalcitTypeAnnotation::String
    | CalcitTypeAnnotation::Unit
    | CalcitTypeAnnotation::JsObject => true,
    CalcitTypeAnnotation::JsNullish(inner) => supported_abi_type(inner),
    _ => false,
  }
}

pub fn validate_resource_path(path: &str) -> Result<(), String> {
  let resource = Path::new(path);
  if resource.is_absolute()
    || path.is_empty()
    || path.contains('\\')
    || path.contains(':')
    || path
      .split('/')
      .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    || resource.components().any(|component| !matches!(component, Component::Normal(_)))
    || !matches!(resource.extension().and_then(|value| value.to_str()), Some("js" | "mjs"))
  {
    return Err(format!(
      "JS FFI resource `{path}` must be a module-root-relative .js/.mjs path without traversal"
    ));
  }
  Ok(())
}

fn is_js_identifier(value: &str) -> bool {
  let mut chars = value.chars();
  matches!(chars.next(), Some(c) if c == '_' || c == '$' || c.is_ascii_alphabetic())
    && chars.all(|c| c == '_' || c == '$' || c.is_ascii_alphanumeric())
}

fn valid_module_specifier(value: &str) -> bool {
  if let Some(builtin) = value.strip_prefix("node:") {
    return !builtin.is_empty()
      && !builtin.contains(['\\', ':'])
      && !builtin.chars().any(char::is_whitespace)
      && builtin.split('/').all(|part| !part.is_empty() && part != "." && part != "..");
  }
  matches!(value.chars().next(), Some('@') | Some('a'..='z') | Some('A'..='Z'))
    && !value.contains(['\\', ':'])
    && !value.chars().any(char::is_whitespace)
    && value.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{CalcitFnTypeAnnotation, SchemaKind};
  use cirru_edn::EdnTag;
  use std::collections::HashSet;
  use std::sync::Arc;

  fn ffi(js: Edn) -> Edn {
    Edn::map_from_iter([(Edn::tag("target"), Edn::tag("node")), (Edn::tag("js"), js)])
  }

  #[test]
  fn parses_inline_and_file_sources() {
    let inline = ffi(Edn::map_from_iter([(Edn::tag("inline"), Edn::str("(x) => x + 1"))]));
    assert_eq!(
      parse_js_source(&inline).unwrap(),
      Some(JsFfiImplementation {
        source: JsFfiSource::Inline("(x) => x + 1".to_owned()),
        modules: vec![]
      })
    );

    let file = ffi(Edn::map_from_iter([(Edn::tag("file"), Edn::str("js/math.mjs"))]));
    assert_eq!(
      parse_js_source(&file).unwrap(),
      Some(JsFfiImplementation {
        source: JsFfiSource::File {
          path: "js/math.mjs".to_owned()
        },
        modules: vec![]
      })
    );
  }

  #[test]
  fn rejects_ambiguous_or_escaping_files() {
    for path in ["../secret.mjs", "/tmp/file.mjs", "js/../../file.mjs", "file.txt", ""] {
      let file = ffi(Edn::map_from_iter([(Edn::tag("file"), Edn::str(path))]));
      assert!(parse_js_source(&file).is_err(), "path should fail: {path}");
    }
    let both = ffi(Edn::map_from_iter([
      (Edn::tag("inline"), Edn::str("() => 1")),
      (Edn::tag("file"), Edn::str("js/file.mjs")),
    ]));
    assert!(parse_js_source(&both).is_err());
    let legacy = ffi(Edn::map_from_iter([
      (Edn::tag("file"), Edn::str("js/file.mjs")),
      (Edn::tag("export"), Edn::str("run")),
    ]));
    assert!(parse_js_source(&legacy).is_err());
  }

  #[test]
  fn explicit_modules_reject_relative_paths() {
    let source = |specifier| {
      ffi(Edn::map_from_iter([
        (Edn::tag("file"), Edn::str("js/file.js")),
        (Edn::tag("modules"), Edn::map_from_iter([(Edn::tag("path"), Edn::str(specifier))])),
      ]))
    };
    assert_eq!(
      parse_js_source(&source("node:path")).unwrap(),
      Some(JsFfiImplementation {
        source: JsFfiSource::File {
          path: "js/file.js".to_owned()
        },
        modules: vec![("path".to_owned(), "node:path".to_owned())],
      })
    );
    for specifier in [
      "./helper.js",
      "../helper.js",
      "/tmp/helper.js",
      "file:///tmp/helper.js",
      "https://example.com/x.js",
      "node:",
      "#private",
    ] {
      assert!(parse_js_source(&source(specifier)).is_err(), "specifier should fail: {specifier}");
    }
  }

  #[test]
  fn source_requires_lexical_feature_and_supported_abi() {
    let source = ffi(Edn::map_from_iter([(Edn::tag("inline"), Edn::str("(x) => x + 1"))]));
    let signature = |features: HashSet<EdnTag>, argument: CalcitTypeAnnotation| {
      CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
        generics: Arc::new(vec![]),
        where_bounds: Arc::new(vec![]),
        arg_types: vec![Arc::new(argument)],
        return_type: Arc::new(CalcitTypeAnnotation::Number),
        fn_kind: SchemaKind::Fn,
        rest_type: None,
        features: Arc::new(features),
      }))
    };
    assert!(validate_js_source(&source, &CalcitTypeAnnotation::Dynamic).is_err());
    assert!(validate_js_source(&source, &signature(HashSet::new(), CalcitTypeAnnotation::Number)).is_err());
    let features = HashSet::from([EdnTag::new("js-ffi")]);
    assert!(
      validate_js_source(
        &source,
        &signature(features.clone(), CalcitTypeAnnotation::List(Arc::new(CalcitTypeAnnotation::Number)))
      )
      .is_err()
    );
    assert!(
      validate_js_source(&source, &signature(features, CalcitTypeAnnotation::Number))
        .unwrap()
        .is_some()
    );
    let native = Edn::map_from_iter([
      (Edn::tag("target"), Edn::tag("native")),
      (Edn::tag("js"), Edn::map_from_iter([(Edn::tag("inline"), Edn::str("(x) => x"))])),
    ]);
    assert!(
      validate_js_source(
        &native,
        &signature(HashSet::from([EdnTag::new("js-ffi")]), CalcitTypeAnnotation::Number)
      )
      .is_err()
    );
  }
}
